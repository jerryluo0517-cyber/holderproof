#!/usr/bin/env python3
"""Replay the fixture on an isolated Anvil, then query archival balanceOf state.

Fixture block N maps to EVM block N+2 (deployment occupies block 1).
EVM hashes and the deployed token address are intentionally different from
synthetic fixture metadata; this check compares balances and Transfer contents.
"""

import itertools
import json
from pathlib import Path
import socket
import subprocess
import time
from urllib.error import URLError
from urllib.request import Request, urlopen

ROOT = Path(__file__).resolve().parents[1]
OWNER = "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266"
OFFSET = 2


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def rpc(url, method, params):
    request = Request(url, json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).encode(),
                      headers={"Content-Type": "application/json"})
    with urlopen(request, timeout=15) as response:
        result = json.load(response)
    if "error" in result:
        raise RuntimeError(f"{method}: {result['error']}")
    return result["result"]


def selector(signature):
    return subprocess.check_output(["cast", "sig", signature], text=True, timeout=10).strip()


def encode_batch(signature, events):
    # ABI encoding of three dynamic arrays: offsets, then length-prefixed data.
    arrays = [[int(event[key], 16) if key != "amount" else int(event[key]) for event in events]
              for key in ["from", "to", "amount"]]
    encoded, offsets = [], []
    offset = 3 * 32
    for values in arrays:
        offsets.append(offset)
        chunk = [len(values), *values]
        encoded.extend(chunk)
        offset += len(chunk) * 32
    return signature + "".join(f"{word:064x}" for word in [*offsets, *encoded])


def receipt(url, tx_hash):
    deadline = time.monotonic() + 15
    while time.monotonic() < deadline:
        result = rpc(url, "eth_getTransactionReceipt", [tx_hash])
        if result:
            require(int(result["status"], 16) == 1, f"Transaction reverted: {tx_hash}")
            return result
        time.sleep(0.05)
    raise RuntimeError(f"Missing receipt: {tx_hash}")


def replay(url, contract, events):
    signature = selector("replay(address[],address[],uint256[])")
    for block, group in itertools.groupby(events, key=lambda event: event["block_number"]):
        batch = list(group)
        target = block + OFFSET
        head = int(rpc(url, "eth_blockNumber", []), 16)
        if target > head + 1:
            rpc(url, "anvil_mine", [hex(target - head - 1)])
        tx_hash = rpc(url, "eth_sendTransaction", [{"from": OWNER, "to": contract,
                      "data": encode_batch(signature, batch), "gas": "0x989680"}])
        result = receipt(url, tx_hash)
        require(int(result["blockNumber"], 16) == target, "Unexpected block mapping")
        require(len(result["logs"]) == len(batch), "Transfer log count mismatch")
        for log, event in zip(result["logs"], batch):
            require(log["topics"][1][-40:] == event["from"][2:] and
                    log["topics"][2][-40:] == event["to"][2:] and
                    int(log["data"], 16) == int(event["amount"]), "Transfer content mismatch")


def verify(url, contract, expected):
    from fixtures.generate_fixture import HOLDERS
    signature = selector("balanceOf(address)")
    results = []
    for snapshot in expected["snapshots"]:
        block = snapshot["block_number"]
        for name, address in {**HOLDERS, "unknown": "0x" + "99" * 20}.items():
            value = rpc(url, "eth_call", [{"to": contract, "data": signature + address[2:].zfill(64)}, hex(block + OFFSET)])
            balance = int(value, 16)
            require(balance == snapshot["balances"].get(name, 0), f"Historical balance mismatch: {name}@{block}")
            results.append({"holder": address, "fixture_block": block, "evm_block": block + OFFSET, "balance": str(balance)})
    certificate_file = ROOT / ".run/step4/certificate.json"
    if certificate_file.exists():
        certificate = json.loads(certificate_file.read_text())
        match = next(item for item in results if item["holder"] == certificate["holder"] and item["fixture_block"] == certificate["block_number"])
        require(match["balance"] == certificate["balance"], "Rust certificate disagrees with historical EVM state")
    return results


def main():
    import sys
    sys.path.insert(0, str(ROOT))
    subprocess.run(["forge", "build", "--root", str(ROOT / "evm"), "--offline"], check=True, timeout=60)
    artifact = json.loads((ROOT / "evm/out/FixtureToken.sol/FixtureToken.json").read_text())
    events = sorted(json.loads((ROOT / "fixtures/transfers.json").read_text()), key=lambda event: (event["block_number"], event["log_index"]))
    expected = json.loads((ROOT / "fixtures/expected_results.json").read_text())
    with socket.socket() as probe:
        probe.bind(("127.0.0.1", 0))
        port = probe.getsockname()[1]
    url = f"http://127.0.0.1:{port}"
    evidence_dir = ROOT / ".run/acceptance"
    evidence_dir.mkdir(parents=True, exist_ok=True)
    with (evidence_dir / "anvil.log").open("w") as log:
        process = subprocess.Popen(["anvil", "--host", "127.0.0.1", "--port", str(port), "--chain-id", "31338", "--silent"], stdout=log, stderr=log)
        try:
            deadline = time.monotonic() + 15
            while True:
                try:
                    rpc(url, "eth_chainId", [])
                    break
                except (URLError, ConnectionError):
                    require(process.poll() is None and time.monotonic() < deadline, "Anvil did not start")
                    time.sleep(0.1)
            tx_hash = rpc(url, "eth_sendTransaction", [{"from": OWNER, "data": artifact["bytecode"]["object"], "gas": "0x989680"}])
            contract = receipt(url, tx_hash)["contractAddress"]
            replay(url, contract, events)
            # The final requested snapshot can follow the last Transfer block.
            final_block = max(item["block_number"] for item in expected["snapshots"]) + OFFSET
            head = int(rpc(url, "eth_blockNumber", []), 16)
            if final_block > head:
                rpc(url, "anvil_mine", [hex(final_block - head)])
            results = verify(url, contract, expected)
            evidence = {"chain_id": 31338, "contract": contract, "fixture_block_offset": OFFSET,
                        "transfer_count": len(events), "checks": results}
            (evidence_dir / "historical-balances.json").write_text(json.dumps(evidence, indent=2) + "\n")
            print(f"[PASS] {len(events)} EVM Transfer logs and {len(results)} archival balanceOf queries agree with Python ground truth")
        finally:
            process.terminate()
            try:
                process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()


if __name__ == "__main__":
    main()
