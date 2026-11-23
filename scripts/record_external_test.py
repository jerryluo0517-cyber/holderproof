#!/usr/bin/env python3
"""Guide an external trial and save a feedback form from the observed results."""

import argparse
from dataclasses import dataclass, field
import json
from pathlib import Path
import platform
import re
import subprocess
import sys
import tempfile
import time
from urllib.request import urlopen

from run_proof import ROOT
from serve_web import GATE_BLOCK as BLOCK
from serve_web import ORIGIN as WEB_URL
from verify_step4 import read_json

sys.path.insert(0, str(ROOT))
from fixtures.generate_fixture import HOLDERS  # noqa: E402

WEB_HOLDERS = ("alice", "bob", "carol", "dave")
GATES = {
    "airdrop": ("空投", 8080, "60000"),
    "governance": ("治理", 8082, "70000"),
}


@dataclass
class Trial:
    tester_id: str
    commit: str
    route: str = "终端"
    holder_name: str = "alice"
    step: str = "环境检查"
    environment_ready: bool = False
    completed: bool = False
    error: str = ""
    certificate: dict = field(default_factory=dict)
    gates: dict[str, dict] = field(default_factory=dict)
    matching_query: bool | None = None


def confirm_step(message: str) -> None:
    while True:
        decision = input(f"\n{message}\n按 Enter 继续，或输入 q 结束试用。").strip().lower()
        if not decision:
            return
        if decision == "q":
            raise RuntimeError("测试者在此步骤结束试用")


def run_step(command: list[str], log_path: Path) -> None:
    with log_path.open("w", encoding="utf-8") as log, subprocess.Popen(
        command, cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
        text=True, encoding="utf-8", errors="replace",
    ) as process:
        for line in process.stdout:
            print(line, end="", flush=True)
            log.write(line)
        returncode = process.wait()
    if returncode:
        raise RuntimeError(f"命令退出码 {returncode}，详见 {log_path.name}")


def run_terminal_query(trial: Trial, run_dir: Path) -> None:
    trial.step = "查询余额证书"
    confirm_step(f"查询 {trial.holder_name.title()} 在 fixture 区块 {BLOCK} 的余额证书。")
    proof_path = run_dir / "certificate.json"
    log_path = run_dir / "query.log"
    with proof_path.open("w", encoding="utf-8") as proof, log_path.open("w", encoding="utf-8") as log:
        result = subprocess.run(
            ["./scripts/run-proof.sh", "--holder", HOLDERS[trial.holder_name], "--block", str(BLOCK)],
            cwd=ROOT, stdout=proof, stderr=log, check=False,
        )
    if result.returncode:
        raise RuntimeError(f"查询退出码 {result.returncode}，详见 query.log")
    trial.certificate = json.loads(proof_path.read_text(encoding="utf-8"))
    if not isinstance(trial.certificate, dict):
        trial.certificate = {}
        raise RuntimeError("查询输出缺少证书对象")
    print(
        f"余额 {trial.certificate.get('balance')}，"
        f"证书状态 {trial.certificate.get('status')}，"
        f"签名者 {signer_count(trial.certificate)} 名。\n"
        f"证书查询哈希 {trial.certificate.get('query_hash')}。"
    )

    trial.step = "核对空投和治理"
    for key, (name, port, _) in GATES.items():
        gate = read_json(port)
        if not isinstance(gate, dict):
            raise RuntimeError(f"{name}接口缺少结果对象")
        trial.gates[key] = gate
        print(f"{name}门槛 {gate['threshold']}，资格 {json.dumps(gate['eligible'])}，查询哈希 {gate['queryHash']}。")
    validate_result(trial)
    confirm_step("请核对终端里的余额证书、两个门槛和资格结果。")


def validate_result(trial: Trial) -> None:
    query_hash = trial.certificate.get("query_hash")
    trial.matching_query = bool(query_hash) and trial.gates.keys() == GATES.keys() and all(
        gate.get("queryHash") == query_hash for gate in trial.gates.values()
    )

    expected = json.loads((ROOT / "fixtures/expected_results.json").read_text(encoding="utf-8"))
    snapshot = next(item for item in expected["snapshots"] if item["block_number"] == BLOCK)
    expected_balance = snapshot["balances"][trial.holder_name]
    if (
        str(trial.certificate.get("holder")).lower() != HOLDERS[trial.holder_name]
        or trial.certificate.get("block_number") != BLOCK
        or trial.certificate.get("balance") != str(expected_balance)
        or trial.certificate.get("status") != "Finalized"
        or signer_count(trial.certificate) not in (3, 4)
        or not trial.matching_query
    ):
        raise RuntimeError("证书或两个 Gate 的查询哈希与试用预期不一致")
    for key, (name, _, threshold) in GATES.items():
        gate = trial.gates[key]
        eligible = expected_balance >= int(threshold)
        if str(gate.get("threshold")) != threshold or gate.get("eligible") is not eligible:
            raise RuntimeError(f"{name}的门槛或资格结果与试用预期不一致")


def read_web_snapshot() -> dict:
    with urlopen(f"{WEB_URL}/api/snapshot", timeout=8) as response:
        return json.load(response)


def wait_for_web(process: subprocess.Popen, log_path: Path) -> dict:
    deadline = time.monotonic() + 90
    while time.monotonic() < deadline and process.poll() is None:
        if f"HolderProof live console: {WEB_URL}" in log_path.read_text(encoding="utf-8", errors="replace"):
            return read_web_snapshot()
        time.sleep(0.25)
    raise RuntimeError("网页服务未就绪，详见 web.log")


def run_web_query(trial: Trial, run_dir: Path) -> None:
    trial.step = "启动网页"
    holder = HOLDERS[trial.holder_name]
    holder_label = trial.holder_name.title()
    log_path = run_dir / "web.log"
    with log_path.open("w", encoding="utf-8") as log, subprocess.Popen(
        [sys.executable, "scripts/serve_web.py"], cwd=ROOT, stdout=log, stderr=subprocess.STDOUT,
    ) as process:
        try:
            initial = wait_for_web(process, log_path)
            initial_counts = {key: initial["gates"][key]["checkCount"] for key in GATES}
            trial.step = "浏览器查询与核对资格"
            while True:
                confirm_step(
                    f"打开 {WEB_URL}/，选择 {holder_label} 和区块 {BLOCK}，点击 Run live query。\n"
                    "看到 Query complete. 后，查看余额证书、空投和治理结果，再回到终端继续。"
                )
                observed = read_web_snapshot()
                certificate = observed["certificate"]
                gates = observed["gates"]
                expected_query = (
                    str(certificate.get("holder")).lower() == holder
                    and certificate.get("block_number") == BLOCK
                )
                fresh_gates = all(gates.get(key, {}).get("checkCount", 0) > initial_counts[key] for key in GATES)
                if expected_query and fresh_gates:
                    break
                print(f"请在网页完成 {holder_label} 在区块 {BLOCK} 的查询，并看到两个资格结果后再继续。")

            trial.certificate = certificate
            trial.gates = gates
            (run_dir / "certificate.json").write_text(
                json.dumps(trial.certificate, indent=2) + "\n", encoding="utf-8",
            )
            validate_result(trial)
        finally:
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()


def run_trial(trial: Trial, run_dir: Path, web: bool) -> None:
    run_step(["./scripts/setup-env.sh", "--check"], run_dir / "environment.log")
    trial.environment_ready = True

    trial.step = "启动 demo"
    confirm_step("启动本地 HolderProof demo。")
    run_step(["make", "demo"], run_dir / "demo.log")

    if web:
        run_web_query(trial, run_dir)
    else:
        run_terminal_query(trial, run_dir)
    trial.completed = True


def signer_count(certificate: dict) -> int | None:
    signers = certificate.get("signer_ids")
    if isinstance(signers, list) and all(isinstance(signer, str) and signer for signer in signers):
        return len(set(signers))
    return None


def collect_feedback() -> tuple[str, str]:
    assistance = {"1": "无协助", "2": "有协助", "": "未记录"}
    try:
        choice = input("\n是否获得协助？输入 1 无协助、2 有协助，或按 Enter 跳过。").strip()
        while choice not in assistance:
            choice = input("请输入 1、2，或按 Enter 跳过。").strip()
        feedback = input("卡点、协助内容或建议（可留空）：").strip()
    except (EOFError, KeyboardInterrupt):
        return "未记录", ""
    return assistance[choice], feedback


def render_form(trial: Trial, elapsed: float, assistance: str, feedback: str) -> str:
    system = platform.mac_ver()[0] or platform.release()
    lines = [
        "# HolderProof 外部用户试用记录",
        "",
        f"- 匿名编号：{trial.tester_id}",
        f"- 测试路线：{trial.route}",
        f"- 查询任务：{trial.holder_name.title()}，区块 {BLOCK}",
        f"- 测试提交：`{trial.commit}`",
        f"- 环境：{platform.system()} {system} / {platform.machine()} / Python {platform.python_version()}",
        f"- 环境检查：{'通过' if trial.environment_ready else '未通过'}",
        f"- 试用耗时（从环境检查开始）：{elapsed:.1f} 秒",
        f"- 完成情况：{'已完成' if trial.completed else '未完成'}",
        f"- 协助情况：{assistance}",
    ]
    if trial.error:
        lines += [f"- 未完成步骤：{trial.step}", f"- 原因：{trial.error}"]
    lines += ["", "## 实际结果", ""]
    if trial.certificate:
        certificate = trial.certificate
        signers = signer_count(certificate)
        lines += [
            f"- 证书区块：{certificate.get('block_number', '未取得')}",
            f"- 余额：{certificate.get('balance', '未取得')}",
            f"- 证书状态：{certificate.get('status', '未取得')}",
            f"- 不同签名者人数：{signers if signers is not None else '未取得'}",
            f"- 查询哈希：`{certificate.get('query_hash', '未取得')}`",
        ]
    else:
        lines.append("- 余额证书：未取得")
    for key, gate in trial.gates.items():
        name = GATES[key][0]
        lines.append(f"- {name}：门槛 {gate.get('threshold')}，资格 `{json.dumps(gate.get('eligible'))}`")
    matching = {True: "一致", False: "不一致", None: "未核对"}
    lines += [
        f"- 两个 Gate 与证书的查询哈希：{matching[trial.matching_query]}",
        "",
        "## 参与者反馈",
        "",
        f"> {feedback or '未填写'}",
    ]
    return "\n".join(lines) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("tester_id", help="团队分配的匿名编号，例如 E01")
    parser.add_argument("--web", action="store_true", help="在浏览器完成查询和资格核对")
    parser.add_argument("--holder", choices=WEB_HOLDERS, help="指定 Web 路线的持有者，默认 Alice")
    args = parser.parse_args()
    if not re.fullmatch(r"[a-zA-Z][a-zA-Z0-9_-]{0,15}", args.tester_id):
        parser.error("请使用团队分配的匿名编号，例如 E01")
    if args.holder and not args.web:
        parser.error("--holder 需要与 --web 一起使用")

    base_dir = ROOT / ".run/external-tests"
    base_dir.mkdir(parents=True, exist_ok=True)
    run_dir = Path(tempfile.mkdtemp(prefix=f"{args.tester_id}-", dir=base_dir))
    form = run_dir / "feedback.md"
    print(f"本轮记录将保存到 {form.relative_to(ROOT)}", flush=True)

    trial = Trial(
        tester_id=args.tester_id,
        commit="未读取",
        route="Web" if args.web else "终端",
        holder_name=args.holder or "alice",
    )
    started = time.monotonic()
    try:
        revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True)
        trial.commit = revision.strip()
        run_trial(trial, run_dir, args.web)
    except (EOFError, KeyboardInterrupt):
        trial.error = "测试者中断试用"
    except (KeyError, OSError, RuntimeError, ValueError, subprocess.CalledProcessError) as error:
        trial.error = str(error)

    elapsed = time.monotonic() - started
    form.write_text(render_form(trial, elapsed, "未记录", ""), encoding="utf-8")
    if trial.error:
        print(f"未完成 {trial.step}：{trial.error}")
    assistance, feedback = collect_feedback()
    form.write_text(render_form(trial, elapsed, assistance, feedback), encoding="utf-8")
    print(f"请把 {form.relative_to(ROOT)} 发给团队。")
    print("保存证据后，如果本轮启动了 demo，请运行 make stop-demo。")
    return 0 if trial.completed else 1


if __name__ == "__main__":
    raise SystemExit(main())
