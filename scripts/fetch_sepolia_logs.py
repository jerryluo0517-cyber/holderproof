#!/usr/bin/env python3
"""Fetch real ERC-20 Transfer logs from Ethereum Sepolia RPC into local fixture format.

Uses only the Python standard library. Saves:
  - fixtures/sepolia_transfers.json
  - fixtures/sepolia_expected.json
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from typing import Any
from urllib.request import Request, urlopen

ROOT = Path(__file__).resolve().parents[1]
CHAIN_ID = 11155111
DATASET_ID = "sepolia-rpc-v1"
PARSER_VERSION = "holderproof-v1"
# keccak256("Transfer(address,address,uint256)")
TRANSFER_TOPIC = "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef"
# Sepolia WETH (active real ERC-20 on Sepolia)
DEFAULT_TOKEN = "0x7b79995e5f793a07bc00c21412e50ecae098e7f9"
DEFAULT_RPC = "https://ethereum-sepolia-rpc.publicnode.com"

EVENT_HASH_FIELDS = (
    "chain_id",
    "token",
    "block_number",
    "block_hash",
    "tx_hash",
    "log_index",
    "from",
    "to",
    "amount",
    "parser_version",
)


def rpc_call(url: str, method: str, params: list[Any]) -> Any:
    body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).encode()
    req = Request(
        url,
        data=body,
        headers={"Content-Type": "application/json", "User-Agent": "HolderProof-SepoliaFetcher/1.0"},
    )
    with urlopen(req, timeout=30) as resp:
        data = json.load(resp)
    if "error" in data:
        raise RuntimeError(f"RPC {method} error: {data['error']}")
    return data["result"]


def event_leaf(event: dict[str, Any]) -> str:
    raw = "|".join(str(event[f]) for f in EVENT_HASH_FIELDS).encode("utf-8")
    return hashlib.sha256(raw).hexdigest()


def merkle_root(leaves: list[str]) -> str:
    layer = list(leaves)
    while len(layer) > 1:
        if len(layer) % 2:
            layer.append(layer[-1])
        layer = [
            hashlib.sha256(bytes.fromhex(layer[i]) + bytes.fromhex(layer[i + 1])).hexdigest()
            for i in range(0, len(layer), 2)
        ]
    return layer[0]


def decode_address(topic: str) -> str:
    return "0x" + topic[-40:].lower()


def fetch_logs(rpc_url: str, token: str, from_block: int, to_block: int, chunk: int) -> list[dict[str, Any]]:
    logs: list[dict[str, Any]] = []
    cur = from_block
    while cur <= to_block:
        end = min(cur + chunk - 1, to_block)
        batch = rpc_call(
            rpc_url,
            "eth_getLogs",
            [{
                "address": token,
                "fromBlock": hex(cur),
                "toBlock": hex(end),
                "topics": [TRANSFER_TOPIC],
            }],
        )
        logs.extend(batch)
        cur = end + 1
    return logs


def replay_nonnegative_holders(events: list[dict[str, Any]]) -> list[tuple[str, int]]:
    """Find holders whose window replay stays strictly non-negative and ends > 0."""
    balances: dict[str, int] = {}
    tainted: set[str] = set()
    for ev in events:
        amt = int(ev["amount"])
        sender = ev["from"]
        receiver = ev["to"]
        if sender != "0x" + "00" * 20:
            new_val = balances.get(sender, 0) - amt
            if new_val < 0:
                tainted.add(sender)
            balances[sender] = new_val
        if receiver != "0x" + "00" * 20:
            balances[receiver] = balances.get(receiver, 0) + amt
    clean = [(addr, bal) for addr, bal in balances.items() if addr not in tainted and bal > 0]
    clean.sort(key=lambda item: (-item[1], item[0]))
    return clean


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rpc-url", default=DEFAULT_RPC)
    parser.add_argument("--token", default=DEFAULT_TOKEN)
    parser.add_argument("--blocks", type=int, default=600, help="Number of recent finalized blocks to scan")
    parser.add_argument("--chunk-size", type=int, default=200)
    args = parser.parse_args()

    token = args.token.lower()
    chain_id = int(rpc_call(args.rpc_url, "eth_chainId", []), 16)
    if chain_id != CHAIN_ID:
        raise RuntimeError(f"Expected Sepolia chain_id {CHAIN_ID}, got {chain_id}")

    latest = int(rpc_call(args.rpc_url, "eth_blockNumber", []), 16)
    # 留 12 个确认块作为已敲定终点
    to_block = latest - 12
    from_block = max(0, to_block - args.blocks + 1)

    print(f"Fetching real Sepolia Transfer logs for {token} in blocks [{from_block}, {to_block}]...")
    raw_logs = fetch_logs(args.rpc_url, token, from_block, to_block, args.chunk_size)
    if not raw_logs:
        raise RuntimeError("No Transfer logs found in this range; try increasing --blocks")

    block_start_info = rpc_call(args.rpc_url, "eth_getBlockByNumber", [hex(from_block), False])
    block_end_info = rpc_call(args.rpc_url, "eth_getBlockByNumber", [hex(to_block), False])

    events: list[dict[str, Any]] = []
    for log in raw_logs:
        if len(log.get("topics", [])) < 3:
            continue
        events.append({
            "chain_id": CHAIN_ID,
            "token": token,
            "block_number": int(log["blockNumber"], 16),
            "block_hash": log["blockHash"].lower(),
            "tx_hash": log["transactionHash"].lower(),
            "log_index": int(log["logIndex"], 16),
            "from": decode_address(log["topics"][1]),
            "to": decode_address(log["topics"][2]),
            "amount": str(int(log["data"], 16)),
            "parser_version": PARSER_VERSION,
        })
    events.sort(key=lambda e: (e["block_number"], e["log_index"]))
    # 去重相同 (block_number, log_index)
    deduped = []
    seen = set()
    for ev in events:
        pos = (ev["block_number"], ev["log_index"])
        if pos not in seen:
            seen.add(pos)
            deduped.append(ev)
    events = deduped

    candidates = replay_nonnegative_holders(events)
    if not candidates:
        raise RuntimeError("No holder with non-negative window inflow found; try increasing --blocks")
    sample_holder, sample_balance = candidates[0]

    # 额外查一下 Sepolia 当前链上真实 balanceOf 作为对比参考
    bal_call_data = "0x70a08231" + sample_holder[2:].zfill(64)
    onchain_latest_balance = int(
        rpc_call(args.rpc_url, "eth_call", [{"to": token, "data": bal_call_data}, "latest"]),
        16,
    )

    root = merkle_root([event_leaf(e) for e in events])
    out_dir = ROOT / "fixtures"
    (out_dir / "sepolia_transfers.json").write_text(json.dumps(events, indent=2) + "\n", encoding="utf-8")
    expected = {
        "dataset_id": DATASET_ID,
        "chain_id": CHAIN_ID,
        "token": token,
        "rpc_url": args.rpc_url,
        "from_block": from_block,
        "to_block": to_block,
        "block_hash_start": block_start_info["hash"].lower(),
        "block_hash": block_end_info["hash"].lower(),
        "event_count": len(events),
        "merkle_root": root,
        "sample_holder": sample_holder,
        "sample_window_balance": str(sample_balance),
        "onchain_latest_balance": str(onchain_latest_balance),
    }
    (out_dir / "sepolia_expected.json").write_text(json.dumps(expected, indent=2) + "\n", encoding="utf-8")

    print(f"[PASS] Fetched {len(events)} real Sepolia Transfer events!")
    print(f"       blocks:        [{from_block}, {to_block}]")
    print(f"       end blockHash: {expected['block_hash']}")
    print(f"       merkle_root:   {root}")
    print(f"       sample_holder: {sample_holder}")
    print(f"       window_bal:    {sample_balance} (on-chain latest balanceOf: {onchain_latest_balance})")


if __name__ == "__main__":
    main()