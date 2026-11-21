#!/usr/bin/env python3
"""Check quorum availability and recovery on the disposable E3 localnet."""

import argparse
from contextlib import contextmanager
import json
import os
import signal
import subprocess
import time

from run_proof import await_certificate, decoded_string, submit_query
from verify_e3 import BLOCK, EVIDENCE, cast, contract_addresses, read_json, require

ENDPOINT = "ws://127.0.0.1:10087/ws"
CERTIFICATE_METHOD = "get_certificate_json(address,uint64) returns (string)"


def localnet_node_pid(port, process_group):
    output = subprocess.check_output(
        ["lsof", "-nP", "-a", "-c", "lyquor", f"-iTCP:{port}", "-sTCP:LISTEN", "-t"],
        text=True,
        timeout=10,
    )
    pids = {int(line) for line in output.splitlines()}
    require(len(pids) == 1, f"Expected one Lyquor listener on port {port}")
    pid = pids.pop()
    require(os.getpgid(pid) == process_group, f"Port {port} is outside the test localnet")
    return pid


@contextmanager
def paused_nodes(pids):
    paused = []
    try:
        for pid in pids:
            os.kill(pid, signal.SIGSTOP)
            paused.append(pid)
        yield
    finally:
        for pid in reversed(paused):
            try:
                os.kill(pid, signal.SIGCONT)
            except ProcessLookupError:
                pass


def read_certificate(indexer, holder):
    result = subprocess.run(
        ["cast", "call", indexer, CERTIFICATE_METHOD, holder, str(BLOCK),
         "--rpc-url", ENDPOINT],
        capture_output=True, text=True, timeout=10,
    )
    if result.returncode:
        require("certificate not found" in result.stderr.lower(),
                f"Unexpected certificate lookup failure: {result.stderr.strip()}")
        return None
    return decoded_string(result.stdout)


def check_query(indexer, holder, token, ports, pids):
    query = argparse.Namespace(holder=holder, token=token, block=BLOCK, endpoint=ENDPOINT)
    require(read_certificate(indexer, holder) is None, "Expected an unused query")
    submit_query(indexer, query)

    with paused_nodes(pids):
        started = time.monotonic()
        try:
            result = subprocess.run(
                ["cast", "call", indexer, "report_balance() returns (bool)", "--rpc-url", ENDPOINT],
                capture_output=True, text=True, timeout=25,
            )
            outcome = (result.stdout if result.returncode == 0 else result.stderr).strip()
        except subprocess.TimeoutExpired:
            outcome = "report_balance timed out after 25 seconds"
        # Let a submitted certification reach the same node used for the read.
        certificate = None
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            certificate = read_certificate(indexer, holder)
            if certificate is not None:
                break
            time.sleep(0.25)
        elapsed = round(time.monotonic() - started, 2)
        if len(pids) == 1:
            require(certificate is not None, f"Two active members did not certify: {outcome}")
            require(certificate["status"] == "Finalized", "Expected a finalized certificate")
            require(len(certificate["signer_ids"]) == 3, "Expected three signers with one paused member")
        else:
            require(certificate is None, "A certificate appeared with fewer than two active members")

    recovered = await_certificate(indexer, query)
    require(recovered["holder"].lower() == holder.lower(), "Wrong holder after recovery")
    require(recovered["status"] == "Finalized" and recovered["balance"] == "0",
            "Expected a finalized zero-balance certificate after recovery")
    if certificate is not None:
        require(recovered == certificate, "Recovery replaced a finalized certificate")
    print(f"[PASS] Paused committee ports {ports}: "
          f"{'certified' if certificate else 'no certificate'} during pause, finalized after recovery")
    return {
        "case": "committee availability",
        "paused_ports": ports,
        "certified_while_paused": certificate is not None,
        "observation_seconds": elapsed,
        "report_outcome": outcome,
        "recovered_certificate": recovered["certificate_id"],
    }


def main():
    state = contract_addresses()
    localnet_group = int(state["LOCALNET_PID"])
    require(os.getpgid(localnet_group) == localnet_group, "Expected the E3 localnet process group")
    node_two = localnet_node_pid(11087, localnet_group)
    node_three = localnet_node_pid(12087, localnet_group)
    _, _, _, holder_one, holder_two, *_ = cast("rpc", "eth_accounts")
    token = read_json(8081, "/api/certificate")["token"]
    indexer = state["INDEXER_CONTRACT"]
    evidence = json.loads(EVIDENCE.read_text())
    evidence.append(check_query(indexer, holder_one, token, [12087], [node_three]))
    evidence.append(check_query(indexer, holder_two, token, [11087, 12087], [node_two, node_three]))
    EVIDENCE.write_text(json.dumps(evidence, indent=2) + "\n")


if __name__ == "__main__":
    main()
