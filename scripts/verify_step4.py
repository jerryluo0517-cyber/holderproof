#!/usr/bin/env python3
"""Verify live facts; optionally exercise both policies on the local demo."""

import argparse
import hashlib
import json
from pathlib import Path
import shlex
import subprocess
import time
from urllib.request import urlopen

ROOT = Path(__file__).resolve().parents[1]
# Public, disposable Anvil development key, never a real wallet.
DEV_KEY = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def read_json(port, path="/api/result"):
    with urlopen(f"http://127.0.0.1:{port}{path}", timeout=10) as response:
        return json.load(response)


def wait_result(port, predicate):
    deadline = time.monotonic() + 30
    while time.monotonic() < deadline:
        result = read_json(port)
        if predicate(result):
            return result
        time.sleep(0.25)
    raise RuntimeError(f"Timed out waiting for gate result on port {port}")


def send(contract, signature, *args):
    subprocess.run(["cast", "send", contract, signature, *map(str, args),
                    "--private-key", DEV_KEY, "--rpc-url", "http://127.0.0.1:8545", "--quiet"],
                   check=True, capture_output=True, text=True, timeout=30)


def exercise_gates(certificate, airdrop):
    state = {}
    for line in (ROOT / ".run/step4/state.env").read_text().splitlines():
        key, value = line.split("=", 1)
        state[key] = shlex.split(value)[0] if value else ""
    cases = []
    for port, contract, threshold, expected in [
        (8080, state["CONSUMER_CONTRACT"], 70000, False),
        (8080, state["CONSUMER_CONTRACT"], 60000, True),
        (8082, state["GOVERNANCE_CONTRACT"], 50000, True),
        (8082, state["GOVERNANCE_CONTRACT"], 70000, False),
    ]:
        signature = "configure(address,address,uint64,uint256)"
        args = [airdrop["indexer"], certificate["holder"], 1000, threshold]
        if port == 8082:
            signature = "configure(address,address,uint64,uint256,uint64)"
            args.append(1)
        send(contract, signature, *args)
        wait_result(port, lambda result: result["threshold"] == str(threshold) and result["checkCount"] == 0)
        send(contract, "check_eligible()")
        result = wait_result(port, lambda result: result["checkCount"] > 0)
        require(result["eligible"] is expected and result["queryHash"] == certificate["query_hash"],
                "Gate did not reuse the expected certificate")
        cases.append(result)
    repeated = json.loads(subprocess.check_output([
        "bash", str(ROOT / "scripts/run-proof.sh"), "--token", certificate["token"],
        "--holder", certificate["holder"], "--block", "1000"], text=True, timeout=90))
    require(repeated == certificate, "Repeated query replaced the original certificate")
    require(read_json(8081, "/api/certificate") == certificate, "Gates changed the shared certificate")
    (ROOT / ".run/step4/gate-cases.json").write_text(json.dumps(cases, indent=2) + "\n")
    for port in [8080, 8082]:
        deadline = time.monotonic() + 10
        while not read_json(port, "/api/history"):
            require(time.monotonic() < deadline, "Missing per-instance result history")
            time.sleep(0.25)
    print("[PASS] Both gates accept/reject; repeated query is idempotent; instance histories are populated")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--exercise-gates", action="store_true", help="Change policies on the local demo to test both outcomes")
    args = parser.parse_args()
    expected = json.loads((ROOT / "fixtures/expected_results.json").read_text())
    certificate = read_json(8081, "/api/certificate")
    airdrop = wait_result(8080, lambda result: result["checkCount"] > 0)
    governance = wait_result(8082, lambda result: result["checkCount"] > 0)
    snapshot = next(item for item in expected["snapshots"] if item["block_number"] == 1000)
    require(int(certificate["balance"]) == snapshot["balances"]["alice"], "Wrong fixture balance")
    require(certificate["checkpoint"]["event_merkle_root"] == expected["merkle_root"], "Wrong Merkle root")
    require(certificate["checkpoint"]["event_count"] == expected["event_count"], "Wrong event count")
    require(certificate["status"] == "Finalized", "Certificate not finalized")
    require(2 <= len(set(certificate["signer_ids"])) == len(certificate["signer_ids"]) <= 3, "Invalid 2-of-3 signer set")
    native = json.loads(certificate["runtime_certificate"])
    require(len(native["signatures"]) == len(certificate["node_signatures"]) == len(certificate["signer_ids"]),
            "Missing native signatures")
    query = "|".join(map(str, ["query", expected["chain_id"], expected["token"], "0x" + "aa" * 20, 1000, "holderproof-v1"]))
    digest = hashlib.sha256(query.encode()).hexdigest()
    require(certificate["query_hash"] == certificate["certificate_id"] == digest, "Wrong query identity")
    for result in [airdrop, governance]:
        require(result["queryHash"] == digest and result["balance"] == certificate["balance"], "Gate fact mismatch")
    require(airdrop["eligible"] is True and governance["eligible"] is False, "Expected different initial gate outcomes")
    (ROOT / ".run/step4/certificate.json").write_text(json.dumps(certificate, indent=2) + "\n")
    print("[PASS] Native certificate and both gate results match fixture ground truth")
    if args.exercise_gates:
        exercise_gates(certificate, airdrop)


if __name__ == "__main__":
    main()
