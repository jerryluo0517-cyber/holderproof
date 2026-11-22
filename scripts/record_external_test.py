#!/usr/bin/env python3
"""Guide an external trial and save a feedback form from the observed results."""

import argparse
from dataclasses import dataclass, field
import json
from pathlib import Path
import platform
import re
import subprocess
import tempfile
import time

from run_proof import ROOT
from verify_step4 import read_json

HOLDER = "0x" + "aa" * 20
BLOCK = 1000
GATES = (("空投", 8080, "60000", True), ("治理", 8082, "70000", False))


@dataclass
class Trial:
    tester_id: str
    commit: str
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


def run_trial(trial: Trial, run_dir: Path) -> None:
    run_step(["./scripts/setup-env.sh", "--check"], run_dir / "environment.log")
    trial.environment_ready = True

    trial.step = "启动 demo"
    confirm_step("启动本地 HolderProof demo。")
    run_step(["make", "demo"], run_dir / "demo.log")

    trial.step = "查询余额证书"
    confirm_step("查询 Alice 在 fixture 区块 1000 的余额证书。")
    proof_path = run_dir / "certificate.json"
    log_path = run_dir / "query.log"
    with proof_path.open("w", encoding="utf-8") as proof, log_path.open("w", encoding="utf-8") as log:
        result = subprocess.run(
            ["./scripts/run-proof.sh", "--holder", HOLDER, "--block", str(BLOCK)],
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
        f"签名者 {signer_count(trial.certificate)} 名。"
    )

    trial.step = "核对空投和治理"
    for name, port, _, _ in GATES:
        print(f"{name}结果 http://127.0.0.1:{port}/api/result")
        gate = read_json(port)
        if not isinstance(gate, dict):
            raise RuntimeError(f"{name}接口缺少结果对象")
        trial.gates[name] = gate
    query_hash = trial.certificate.get("query_hash")
    trial.matching_query = bool(query_hash) and all(
        gate.get("queryHash") == query_hash for gate in trial.gates.values()
    )

    expected = json.loads((ROOT / "fixtures/expected_results.json").read_text(encoding="utf-8"))
    snapshot = next(item for item in expected["snapshots"] if item["block_number"] == BLOCK)
    if (
        str(trial.certificate.get("holder")).lower() != HOLDER
        or trial.certificate.get("block_number") != BLOCK
        or trial.certificate.get("balance") != str(snapshot["balances"]["alice"])
        or trial.certificate.get("status") != "Finalized"
        or signer_count(trial.certificate) not in (3, 4)
        or not trial.matching_query
    ):
        raise RuntimeError("证书或两个 Gate 的查询哈希与试用预期不一致")
    for name, _, threshold, eligible in GATES:
        gate = trial.gates[name]
        if str(gate.get("threshold")) != threshold or gate.get("eligible") is not eligible:
            raise RuntimeError(f"{name}的门槛或资格结果与试用预期不一致")

    confirm_step("请在浏览器打开上面两个地址，查看门槛和 eligible 字段。核对后继续。")
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
    for name, gate in trial.gates.items():
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
    args = parser.parse_args()
    if not re.fullmatch(r"[a-zA-Z][a-zA-Z0-9_-]{0,15}", args.tester_id):
        parser.error("请使用团队分配的匿名编号，例如 E01")

    base_dir = ROOT / ".run/external-tests"
    base_dir.mkdir(parents=True, exist_ok=True)
    run_dir = Path(tempfile.mkdtemp(prefix=f"{args.tester_id}-", dir=base_dir))
    form = run_dir / "feedback.md"
    print(f"本轮记录将保存到 {form.relative_to(ROOT)}", flush=True)

    trial = Trial(tester_id=args.tester_id, commit="未读取")
    started = time.monotonic()
    try:
        revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True)
        trial.commit = revision.strip()
        run_trial(trial, run_dir)
    except (EOFError, KeyboardInterrupt):
        trial.error = "测试者中断试用"
    except (OSError, RuntimeError, ValueError, subprocess.CalledProcessError) as error:
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
