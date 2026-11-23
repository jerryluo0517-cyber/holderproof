#!/usr/bin/env python3
"""End-to-end verification for the VeriCredit-Idx enhancement scenario."""

import json
from pathlib import Path
import shlex
import subprocess
import time
from urllib.error import URLError
from urllib.request import urlopen

from lyquor_env import lyquor_command

ROOT = Path(__file__).resolve().parents[1]
RUN_DIR = ROOT / ".run/step4"
EVIDENCE = ROOT / ".run/acceptance/credit-results.json"
WS_RPC = "ws://127.0.0.1:10087/ws"
ANVIL = "http://127.0.0.1:8545"
DEV_KEY = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
CREDIT_PORT = 8083

EXPECTED_CASES = [
    ("alice", "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", 940, "VIP", 5, 8500, "0"),
    ("dave",  "0xdddddddddddddddddddddddddddddddddddddddd", 675, "Standard", 15, 7500, "0"),
    ("bob",   "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", 495, "Base", 30, 6000, "2000000"),
    ("carol", "0xcccccccccccccccccccccccccccccccccccccccc", 250, "Base", 30, 6000, "0"),
]


def require(cond, msg):
    if not cond:
        raise RuntimeError(msg)


def run(cmd, timeout=60):
    return subprocess.check_output(
        lyquor_command(*cmd), text=True, stderr=subprocess.PIPE, timeout=timeout,
    ).strip()


def read_json(port, path="/api/credit"):
    with urlopen(f"http://127.0.0.1:{port}{path}", timeout=10) as resp:
        return json.load(resp)


def wait_url(url, label):
    deadline = time.monotonic() + 30
    while time.monotonic() < deadline:
        try:
            with urlopen(url, timeout=5):
                return
        except (URLError, ConnectionError):
            time.sleep(0.5)
    raise RuntimeError(f"Timed out waiting for {label} at {url}")


def wait_credit(predicate, label):
    deadline = time.monotonic() + 30
    while time.monotonic() < deadline:
        data = read_json(CREDIT_PORT, "/api/credit")
        if predicate(data):
            return data
        time.sleep(0.25)
    raise RuntimeError(f"Timed out waiting for {label}")


def read_state():
    state = {}
    for line in (RUN_DIR / "state.env").read_text().splitlines():
        if "=" in line:
            k, v = line.split("=", 1)
            state[k] = shlex.split(v)[0] if v else ""
    return state


def parse_last_json(raw):
    for line in reversed(raw.splitlines()):
        try:
            return json.loads(line)
        except json.JSONDecodeError:
            continue
    raise RuntimeError(f"Could not parse JSON from output:\n{raw}")


def activate_credit_epoch(indexer_contract, indexer_target):
    def current_epoch():
        out = run([
            "cast", "call", "--rpc-url", WS_RPC, indexer_contract,
            "__lyquor_oracle_dest_epoch_info(string,bool) returns (uint64,bytes32,uint32,bytes)",
            "credit_proof", "false",
        ])
        return int(out.splitlines()[0].strip())

    if current_epoch() == 0:
        run([
            "cast", "call", "--rpc-url", WS_RPC, indexer_contract,
            "__lyquor_oracle_advance_epoch(string,address,bool) returns (bool)",
            "credit_proof", indexer_target, "false",
        ])
        deadline = time.monotonic() + 60
        while time.monotonic() < deadline:
            if current_epoch() > 0:
                break
            time.sleep(1)
        require(current_epoch() > 0, "credit_proof epoch did not advance")
        run([
            "cast", "call", "--rpc-url", WS_RPC, indexer_contract,
            "__lyquor_oracle_finalize_epoch(string,address,bool) returns (bool)",
            "credit_proof", indexer_target, "false",
        ])


def certify_and_quote(indexer_contract, gate_contract, user, prev_count):
    run([
        "cast", "send", indexer_contract, "set_credit_query(address,uint64,uint64)",
        user, "0", "1000", "--private-key", DEV_KEY, "--rpc-url", WS_RPC, "--quiet",
    ])
    time.sleep(1)
    run(["cast", "call", indexer_contract, "report_credit() returns (bool)", "--rpc-url", WS_RPC])

    deadline = time.monotonic() + 60
    cert_raw = None
    while time.monotonic() < deadline:
        try:
            cert_raw = run([
                "cast", "call", indexer_contract,
                "get_credit_certificate_json(address,uint64,uint64) returns (string)",
                user, "0", "1000", "--rpc-url", WS_RPC,
            ])
            break
        except subprocess.CalledProcessError:
            time.sleep(0.5)
    require(cert_raw is not None, f"Timed out waiting for credit certificate of {user}")
    val = json.loads(cert_raw)
    cert = json.loads(val) if isinstance(val, str) else val

    run([
        "cast", "send", gate_contract, "quote_for(address)", user,
        "--private-key", DEV_KEY, "--rpc-url", ANVIL, "--quiet",
    ])
    quote = wait_credit(lambda d: d["quoteCount"] == prev_count + 1, f"quote for {user}")
    return cert, quote


def main():
    state = read_state()
    indexer_contract = state["INDEXER_CONTRACT"]
    indexer_target = read_json(8080, "/api/result")["indexer"]

    print("[1/5] Deploying VeriCreditGateLyquid...")
    deploy_out = run([
        "shaker", "deploy", "--endpoint", WS_RPC,
        "--reference", "http://127.0.0.1:10087/lyquids:holderproof-credit-gate-local",
        "--private-key", DEV_KEY, "--output", "json",
        str(ROOT / "lyquid-credit-gate/Cargo.toml"),
    ], timeout=120)
    deployed = parse_last_json(deploy_out)
    gate_lyquid = deployed["lyquid_id"]
    gate_contract = deployed["contract"]
    print(f"      {gate_lyquid} -> {gate_contract}")

    print("[2/5] Activating credit_proof oracle epoch & configuring gate...")
    activate_credit_epoch(indexer_contract, indexer_target)
    run([
        "cast", "send", gate_contract, "configure(address,uint64,uint64)",
        indexer_target, "0", "1000", "--private-key", DEV_KEY, "--rpc-url", ANVIL, "--quiet",
    ])

    serve_log = (RUN_DIR / "serve-credit.log").open("w")
    serve_proc = subprocess.Popen(
        lyquor_command("shaker", "serve", "--endpoint", WS_RPC, "--listen", f"127.0.0.1:{CREDIT_PORT}", gate_lyquid),
        stdout=serve_log, stderr=serve_log,
    )
    try:
        wait_url(f"http://127.0.0.1:{CREDIT_PORT}/lyquid/info", "VeriCreditGate proxy")
        wait_credit(lambda d: d["indexer"].lower() == indexer_target.lower(), "gate configuration")

        print("[3/5] Certifying & quoting 4 personas (default weights 35/20/2/3)...")
        evidence = []
        count = 0
        for name, addr, exp_score, exp_tier, exp_fee, exp_ltv, exp_wash in EXPECTED_CASES:
            cert, quote = certify_and_quote(indexer_contract, gate_contract, addr, count)
            count += 1
            require(quote["score"] == exp_score, f"{name} score {quote['score']} != {exp_score}")
            require(quote["tier"] == exp_tier, f"{name} tier {quote['tier']} != {exp_tier}")
            require(quote["feeBps"] == exp_fee, f"{name} fee {quote['feeBps']} != {exp_fee}")
            require(quote["ltvBps"] == exp_ltv, f"{name} ltv {quote['ltvBps']} != {exp_ltv}")
            require(quote["washRemoved"] == exp_wash, f"{name} wash {quote['washRemoved']} != {exp_wash}")
            print(f"      [PASS] {name:5s}: score={quote['score']:>3} tier={quote['tier']:8s} "
                  f"fee={quote['feeBps']:>2}bps ltv={quote['ltvBps']}bps wash_removed={quote['washRemoved']}")
            evidence.append({"case": f"default_{name}", "quote": quote, "certificate_id": cert["certificate_id"]})

        print("[4/5] Testing custom credit weights & non-initializer access control...")
        bob = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
        non_owner = json.loads(run(["cast", "rpc", "eth_accounts", "--rpc-url", ANVIL]))[1]
        # 非部署者越权修改权重应被忽略
        run([
            "cast", "send", "--unlocked", "--from", non_owner, indexer_contract,
            "configure_credit_params(uint64,uint64,uint64,uint64)", "100", "100", "0", "0",
            "--rpc-url", ANVIL, "--quiet",
        ])
        # 部署者把洗盘窗口设为 0，Bob 跨块对敲不再剔除，分数应从 495 升至 740 (Standard)
        run([
            "cast", "send", indexer_contract,
            "configure_credit_params(uint64,uint64,uint64,uint64)", "35", "20", "2", "0",
            "--private-key", DEV_KEY, "--rpc-url", WS_RPC, "--quiet",
        ])
        time.sleep(1)
        bob_lax_cert, bob_lax_quote = certify_and_quote(indexer_contract, gate_contract, bob, count)
        require(bob_lax_quote["score"] == 740 and bob_lax_quote["tier"] == "Standard",
                f"Expected Bob score 740/Standard with wash_window=0, got {bob_lax_quote}")
        require(bob_lax_cert["query_hash"] != evidence[2]["certificate_id"],
                "Custom weights must produce a distinct query_hash")
        print(f"      [PASS] custom wash_window=0 -> bob score jumped 495 -> {bob_lax_quote['score']} "
              f"({bob_lax_quote['tier']}, fee={bob_lax_quote['feeBps']}bps)")
        evidence.append({"case": "custom_params_bob_wash0", "quote": bob_lax_quote, "certificate_id": bob_lax_cert["certificate_id"]})

        # 恢复默认参数
        run([
            "cast", "send", indexer_contract,
            "configure_credit_params(uint64,uint64,uint64,uint64)", "35", "20", "2", "3",
            "--private-key", DEV_KEY, "--rpc-url", WS_RPC, "--quiet",
        ])

        print("[5/5] Checking /api/history & writing evidence...")
        history = read_json(CREDIT_PORT, "/api/history")
        require(len(history) >= 5, f"Expected >=5 history items, got {len(history)}")
        EVIDENCE.parent.mkdir(parents=True, exist_ok=True)
        EVIDENCE.write_text(json.dumps(evidence, indent=2) + "\n")
        print(f"[PASS] VeriCredit-Idx E2E verified! Evidence written to {EVIDENCE.relative_to(ROOT)}")
    finally:
        serve_proc.terminate()
        try:
            serve_proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            serve_proc.kill()
        serve_log.close()


if __name__ == "__main__":
    main()
