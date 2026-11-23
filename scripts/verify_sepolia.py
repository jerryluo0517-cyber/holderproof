#!/usr/bin/env python3
"""Verify real Sepolia Transfer logs through the 3-of-4 sepolia_proof committee."""

import json
from pathlib import Path
import shlex
import subprocess
import time
from urllib.request import urlopen

from lyquor_env import lyquor_command

ROOT = Path(__file__).resolve().parents[1]
RUN_DIR = ROOT / ".run/step4"
EVIDENCE = ROOT / ".run/acceptance/sepolia-certificate.json"
WS_RPC = "ws://127.0.0.1:10087/ws"
DEV_KEY = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"


def require(cond, msg):
    if not cond:
        raise RuntimeError(msg)


def run(cmd, timeout=60):
    return subprocess.check_output(
        lyquor_command(*cmd), text=True, stderr=subprocess.PIPE, timeout=timeout,
    ).strip()


def read_json(port, path):
    with urlopen(f"http://127.0.0.1:{port}{path}", timeout=10) as resp:
        return json.load(resp)


def read_indexer_contract():
    for line in (RUN_DIR / "state.env").read_text().splitlines():
        if line.startswith("INDEXER_CONTRACT="):
            return shlex.split(line.split("=", 1)[1])[0]
    raise RuntimeError("INDEXER_CONTRACT not found")


def activate_sepolia_epoch(indexer_contract, indexer_target):
    def current_epoch():
        out = run([
            "cast", "call", "--rpc-url", WS_RPC, indexer_contract,
            "__lyquor_oracle_dest_epoch_info(string,bool) returns (uint64,bytes32,uint32,bytes)",
            "sepolia_proof", "false",
        ])
        return int(out.splitlines()[0].strip())

    if current_epoch() == 0:
        run([
            "cast", "call", "--rpc-url", WS_RPC, indexer_contract,
            "__lyquor_oracle_advance_epoch(string,address,bool) returns (bool)",
            "sepolia_proof", indexer_target, "false",
        ])
        deadline = time.monotonic() + 60
        while time.monotonic() < deadline and current_epoch() == 0:
            time.sleep(1)
        require(current_epoch() > 0, "sepolia_proof epoch did not advance")
        run([
            "cast", "call", "--rpc-url", WS_RPC, indexer_contract,
            "__lyquor_oracle_finalize_epoch(string,address,bool) returns (bool)",
            "sepolia_proof", indexer_target, "false",
        ])


def main():
    expected = json.loads((ROOT / "fixtures/sepolia_expected.json").read_text())
    indexer_contract = read_indexer_contract()
    indexer_target = read_json(8080, "/api/result")["indexer"]
    holder = expected["sample_holder"]

    print("[1/3] Activating sepolia_proof 3-of-4 oracle epoch...")
    activate_sepolia_epoch(indexer_contract, indexer_target)

    print(f"[2/3] Certifying real Sepolia WETH balance for {holder} at block {expected['to_block']}...")
    run([
        "cast", "send", indexer_contract, "set_sepolia_query(address)", holder,
        "--private-key", DEV_KEY, "--rpc-url", WS_RPC, "--quiet",
    ])
    time.sleep(1)
    run(["cast", "call", indexer_contract, "report_sepolia_balance() returns (bool)", "--rpc-url", WS_RPC])

    deadline = time.monotonic() + 60
    raw = None
    while time.monotonic() < deadline:
        try:
            raw = run([
                "cast", "call", indexer_contract,
                "get_sepolia_certificate_json(address) returns (string)", holder,
                "--rpc-url", WS_RPC,
            ])
            break
        except subprocess.CalledProcessError:
            time.sleep(0.5)
    require(raw is not None, "Timed out waiting for Sepolia certificate")
    val = json.loads(raw)
    cert = json.loads(val) if isinstance(val, str) else val

    print("[3/3] Verifying certificate fields & HTTP /api/sepolia-certificate...")
    require(cert["status"] == "Finalized", "Sepolia cert not Finalized")
    require(3 <= len(cert["signer_ids"]) <= 4, f"Expected 3-of-4 signers, got {len(cert['signer_ids'])}")
    require(cert["balance"] == expected["sample_window_balance"], "Balance mismatch")
    require(cert["checkpoint"]["block_hash"] == expected["block_hash"], "Real Sepolia block_hash mismatch")
    require(cert["checkpoint"]["event_merkle_root"] == expected["merkle_root"], "Merkle root mismatch")
    http_cert = read_json(8081, "/api/sepolia-certificate")
    require(http_cert == cert, "HTTP /api/sepolia-certificate disagrees with on-chain cert")

    EVIDENCE.parent.mkdir(parents=True, exist_ok=True)
    EVIDENCE.write_text(json.dumps(cert, indent=2) + "\n")
    print(f"[PASS] Real Sepolia certificate signed by {len(cert['signer_ids'])} nodes!")
    print(f"       block={cert['block_number']} hash={cert['checkpoint']['block_hash']}")
    print(f"       balance={cert['balance']} events={cert['checkpoint']['event_count']}")
    print(f"       evidence -> {EVIDENCE.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
