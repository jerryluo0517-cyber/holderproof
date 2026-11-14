#!/usr/bin/env python3
"""Submit a P0 fixture query to the local Indexer and print its certificate."""

import argparse
import json
import os
from pathlib import Path
import shlex
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]
TOKEN = "0x5298000000000000000000000000000000000001"
DEV_KEY = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"


def cast(*args):
    return subprocess.check_output(["cast", *map(str, args)], text=True, stderr=subprocess.PIPE, timeout=30).strip()


def decoded_string(raw):
    value = json.loads(raw)
    return json.loads(value) if isinstance(value, str) else value


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--token", default=TOKEN)
    parser.add_argument("--holder", required=True)
    parser.add_argument("--block", type=int, required=True)
    parser.add_argument("--endpoint", default="ws://127.0.0.1:10087/ws")
    args = parser.parse_args()
    if args.token.lower() != TOKEN or not 0 <= args.block <= 1000:
        parser.error("P0 supports the fixture token and finalized fixture blocks 0..1000")
    try:
        if len(args.holder) != 42 or not args.holder.startswith("0x") or int(args.holder, 16) == 0:
            raise ValueError()
    except ValueError:
        parser.error("holder must be a nonzero 20-byte hex address")
    state = {}
    for line in (ROOT / ".run/step4/state.env").read_text().splitlines():
        key, value = line.split("=", 1)
        state[key] = shlex.split(value)[0] if value else ""
    indexer = state["INDEXER_CONTRACT"]
    cast("send", indexer, "proof(address,address,uint64)", args.token, args.holder, args.block,
         "--private-key", os.environ.get("HOLDERPROOF_DEV_KEY", DEV_KEY), "--rpc-url", args.endpoint, "--quiet")
    deadline = time.monotonic() + 30
    while True:
        query = decoded_string(cast("call", indexer, "get_query_json() returns (string)", "--rpc-url", args.endpoint))
        if query["holder"].lower() == args.holder.lower() and query["block"] == args.block:
            break
        if time.monotonic() >= deadline:
            raise RuntimeError("Indexer did not select the requested query")
        time.sleep(0.25)
    cast("call", indexer, "report_balance() returns (bool)", "--rpc-url", args.endpoint)
    deadline = time.monotonic() + 60
    while time.monotonic() < deadline:
        try:
            certificate = decoded_string(cast("call", indexer,
                "get_certificate_json(address,uint64) returns (string)", args.holder, args.block,
                "--rpc-url", args.endpoint))
            print(json.dumps(certificate, indent=2))
            return
        except subprocess.CalledProcessError:
            time.sleep(0.5)
    raise RuntimeError("Committee did not produce a certificate before timeout")


if __name__ == "__main__":
    main()
