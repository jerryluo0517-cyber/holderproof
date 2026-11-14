#!/usr/bin/env python3
"""Submit a P0 fixture query to the local Indexer and print its certificate."""

import argparse
import json
import os
from pathlib import Path
import shlex
import subprocess
import time
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
TOKEN = "0x5298000000000000000000000000000000000001"
# Public disposable Anvil key, used only for local development.
DEV_KEY = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"


def cast(*arguments: object) -> str:
    """Run cast without shell interpolation and return its textual result."""
    return subprocess.check_output(
        ["cast", *map(str, arguments)],
        text=True,
        stderr=subprocess.PIPE,
        timeout=30,
    ).strip()


def decoded_string(raw: str) -> dict[str, Any]:
    """Decode cast's ABI string wrapper and its JSON payload."""
    value = json.loads(raw)
    return json.loads(value) if isinstance(value, str) else value


def parse_arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--token", default=TOKEN)
    parser.add_argument("--holder", required=True)
    parser.add_argument("--block", type=int, required=True)
    parser.add_argument("--endpoint", default="ws://127.0.0.1:10087/ws")
    arguments = parser.parse_args()
    if arguments.token.lower() != TOKEN or not 0 <= arguments.block <= 1000:
        parser.error("P0 supports the fixture token and finalized fixture blocks 0..1000")
    try:
        holder = arguments.holder
        if len(holder) != 42 or not holder.startswith("0x") or int(holder, 16) == 0:
            raise ValueError()
    except ValueError:
        parser.error("holder must be a nonzero 20-byte hex address")
    return arguments


def read_indexer_address() -> str:
    """Read the deployment produced by reproduce-step4.sh; never execute it."""
    state = {}
    for line in (ROOT / ".run/step4/state.env").read_text().splitlines():
        key, value = line.split("=", 1)
        state[key] = shlex.split(value)[0] if value else ""
    return state["INDEXER_CONTRACT"]


def submit_query(indexer: str, arguments: argparse.Namespace) -> None:
    cast(
        "send", indexer, "proof(address,address,uint64)",
        arguments.token, arguments.holder, arguments.block,
        "--private-key", os.environ.get("HOLDERPROOF_DEV_KEY", DEV_KEY),
        "--rpc-url", arguments.endpoint, "--quiet",
    )
    deadline = time.monotonic() + 30
    while time.monotonic() < deadline:
        query = decoded_string(cast(
            "call", indexer, "get_query_json() returns (string)",
            "--rpc-url", arguments.endpoint,
        ))
        if query["holder"].lower() == arguments.holder.lower() and query["block"] == arguments.block:
            return
        time.sleep(0.25)
    raise RuntimeError("Indexer did not select the requested query")


def await_certificate(indexer: str, arguments: argparse.Namespace) -> dict[str, Any]:
    cast("call", indexer, "report_balance() returns (bool)", "--rpc-url", arguments.endpoint)
    deadline = time.monotonic() + 60
    while time.monotonic() < deadline:
        try:
            return decoded_string(cast(
                "call", indexer, "get_certificate_json(address,uint64) returns (string)",
                arguments.holder, arguments.block, "--rpc-url", arguments.endpoint,
            ))
        except subprocess.CalledProcessError:
            time.sleep(0.5)
    raise RuntimeError("Committee did not produce a certificate before timeout")


def main() -> None:
    arguments = parse_arguments()
    indexer = read_indexer_address()
    submit_query(indexer, arguments)
    certificate = await_certificate(indexer, arguments)
    print(json.dumps(certificate, indent=2))


if __name__ == "__main__":
    main()
