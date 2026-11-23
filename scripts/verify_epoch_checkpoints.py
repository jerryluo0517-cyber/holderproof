#!/usr/bin/env python3
"""Commit and verify periodic global Epoch Checkpoints (250/500/750/1000) via 3-of-4 quorum."""

import json
from pathlib import Path
import shlex
import subprocess
import time
from urllib.request import urlopen

from lyquor_env import lyquor_command

ROOT = Path(__file__).resolve().parents[1]
RUN_DIR = ROOT / ".run/step4"
EVIDENCE = ROOT / ".run/acceptance/epoch-checkpoints.json"
WS_RPC = "ws://127.0.0.1:10087/ws"
DEV_KEY = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
EPOCH_BLOCKS = [250, 500, 750, 1000]


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
    raise RuntimeError("INDEXER_CONTRACT not found in .run/step4/state.env")


def activate_checkpoint_epoch(indexer_contract, indexer_target):
    def current_epoch():
        out = run([
            "cast", "call", "--rpc-url", WS_RPC, indexer_contract,
            "__lyquor_oracle_dest_epoch_info(string,bool) returns (uint64,bytes32,uint32,bytes)",
            "checkpoint_proof", "false",
        ])
        return int(out.splitlines()[0].strip())

    if current_epoch() == 0:
        run([
            "cast", "call", "--rpc-url", WS_RPC, indexer_contract,
            "__lyquor_oracle_advance_epoch(string,address,bool) returns (bool)",
            "checkpoint_proof", indexer_target, "false",
        ])
        deadline = time.monotonic() + 60
        while time.monotonic() < deadline and current_epoch() == 0:
            time.sleep(1)
        require(current_epoch() > 0, "checkpoint_proof epoch did not advance")
        run([
            "cast", "call", "--rpc-url", WS_RPC, indexer_contract,
            "__lyquor_oracle_finalize_epoch(string,address,bool) returns (bool)",
            "checkpoint_proof", indexer_target, "false",
        ])


def main():
    indexer_contract = read_indexer_contract()
    indexer_target = read_json(8080, "/api/result")["indexer"]
    expected = json.loads((ROOT / "fixtures/expected_results.json").read_text())

    print("[1/3] Activating checkpoint_proof oracle epoch...")
    activate_checkpoint_epoch(indexer_contract, indexer_target)

    print("[2/3] Committing periodic global checkpoints (250, 500, 750, 1000)...")
    for blk in EPOCH_BLOCKS:
        run([
            "cast", "send", indexer_contract, "set_epoch_target(uint64)", str(blk),
            "--private-key", DEV_KEY, "--rpc-url", WS_RPC, "--quiet",
        ])
        time.sleep(1)
        run(["cast", "call", indexer_contract, "report_epoch_checkpoint() returns (bool)", "--rpc-url", WS_RPC])

        deadline = time.monotonic() + 60
        ep_raw = None
        while time.monotonic() < deadline:
            try:
                ep_raw = run([
                    "cast", "call", indexer_contract,
                    "get_epoch_checkpoint_json(uint64) returns (string)", str(blk),
                    "--rpc-url", WS_RPC,
                ])
                break
            except subprocess.CalledProcessError:
                time.sleep(0.5)
        require(ep_raw is not None, f"Timed out waiting for epoch checkpoint at block {blk}")
        val = json.loads(ep_raw)
        ep = json.loads(val) if isinstance(val, str) else val
        require(3 <= len(ep["signer_ids"]) <= 4, f"Expected 3-of-4 signers at block {blk}")
        print(f"      [PASS] Epoch #{ep['epoch_id']} (block {blk:>4}): "
              f"events={ep['checkpoint']['event_count']:>3} "
              f"volume={ep['total_transfer_volume']:>7} "
              f"signers={len(ep['signer_ids'])}")

    print("[3/3] Checking HTTP /api/epoch-checkpoints & saving evidence...")
    summary = read_json(8081, "/api/epoch-checkpoints")
    require(summary["latestEpochBlock"] == 1000 and len(summary["checkpoints"]) == 4, "Missing checkpoints")
    last = summary["checkpoints"][-1]
    require(last["checkpoint"]["event_merkle_root"] == expected["merkle_root"], "Epoch 4 root mismatch")
    EVIDENCE.parent.mkdir(parents=True, exist_ok=True)
    EVIDENCE.write_text(json.dumps(summary, indent=2) + "\n")
    print(f"[PASS] All 4 global Epoch Checkpoints verified! Evidence written to {EVIDENCE.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
