#!/usr/bin/env python3
"""Generate deterministic VeriCredit behavior events and ground-truth scores."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any

ZERO_ADDRESS = "0x" + "00" * 20
CREDIT_PROTOCOL = "0x5298000000000000000000000000000000000002"
CHAIN_ID = 11155111
PARSER_VERSION = "holderproof-v1"
CREDIT_DATASET_ID = "vericredit-fixture-5298-v1"
DEFAULT_END_BLOCK = 1000

DEFAULT_W1 = 35
DEFAULT_W2 = 20
DEFAULT_ALPHA = 2
DEFAULT_WASH_WINDOW = 3

USERS = {
    "alice": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "bob": "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
    "carol": "0xcccccccccccccccccccccccccccccccccccccccc",
    "dave": "0xdddddddddddddddddddddddddddddddddddddddd",
}

POOL_A = "0x1111111111111111111111111111111111111111"
POOL_B = "0x2222222222222222222222222222222222222222"

CREDIT_HASH_FIELDS = (
    "chain_id",
    "protocol",
    "block_number",
    "block_hash",
    "tx_hash",
    "log_index",
    "user",
    "counterparty",
    "event_type",
    "amount",
    "parser_version",
)


def _hex_digest(*parts: object) -> str:
    raw = "|".join(str(p) for p in parts).encode("utf-8")
    return hashlib.sha256(raw).hexdigest()


def block_hash(block_number: int) -> str:
    return "0x" + _hex_digest("block", CHAIN_ID, block_number)


def make_event(
    block_number: int,
    log_index: int,
    user: str,
    counterparty: str,
    event_type: str,
    amount: int,
    idx: int,
) -> dict[str, Any]:
    return {
        "chain_id": CHAIN_ID,
        "protocol": CREDIT_PROTOCOL,
        "block_number": block_number,
        "block_hash": block_hash(block_number),
        "tx_hash": "0x" + _hex_digest("credit_tx", 5298, idx),
        "log_index": log_index,
        "user": user.lower(),
        "counterparty": counterparty.lower(),
        "event_type": event_type,
        "amount": str(amount),
        "parser_version": PARSER_VERSION,
    }


def generate_credit_events() -> list[dict[str, Any]]:
    raw: list[tuple[int, int, str, str, str, int]] = []

    # Alice: 诚实 VIP 用户 (clean_volume = 1_200_000, repay = 12, liq = 0 -> score = 940)
    for i in range(6):
        raw.append((100 + i * 50, 0, USERS["alice"], POOL_A, "SwapBuy", 200_000))
    for i in range(12):
        raw.append((120 + i * 60, 1, USERS["alice"], ZERO_ADDRESS, "Repay", 10_000))

    # Bob: 洗盘刷量用户 (2_000_000 在 1~2 块内反向对敲被剔除, clean = 15_000, repay = 2 -> score = 495)
    raw.append((200, 0, USERS["bob"], POOL_A, "SwapBuy", 500_000))
    raw.append((201, 0, USERS["bob"], POOL_A, "SwapSell", 500_000))  # 间隔 1 块 <= 3，洗盘抵消
    raw.append((300, 0, USERS["bob"], POOL_B, "SwapBuy", 500_000))
    raw.append((302, 0, USERS["bob"], POOL_B, "SwapSell", 500_000))  # 间隔 2 块 <= 3，洗盘抵消
    raw.append((450, 0, USERS["bob"], POOL_A, "SwapBuy", 15_000))    # 真实交易
    raw.append((500, 0, USERS["bob"], ZERO_ADDRESS, "Repay", 5_000))
    raw.append((600, 0, USERS["bob"], ZERO_ADDRESS, "Repay", 5_000))

    # Carol: 有清算惩罚用户 (clean_volume = 500_000, repay = 6, liq = 1 -> score = 250)
    raw.append((250, 0, USERS["carol"], POOL_A, "SwapBuy", 300_000))
    raw.append((550, 0, USERS["carol"], POOL_B, "SwapBuy", 200_000))
    for i in range(6):
        raw.append((310 + i * 80, 1, USERS["carol"], ZERO_ADDRESS, "Repay", 8_000))
    raw.append((800, 0, USERS["carol"], ZERO_ADDRESS, "Liquidation", 50_000))

    # Dave: 标准中档用户 (clean_volume = 200_000, repay = 4, liq = 0 -> score = 675)
    raw.append((150, 0, USERS["dave"], POOL_B, "SwapBuy", 120_000))
    raw.append((400, 0, USERS["dave"], POOL_A, "SwapBuy", 80_000))
    for i in range(4):
        raw.append((220 + i * 150, 1, USERS["dave"], ZERO_ADDRESS, "Repay", 6_000))

    raw.sort(key=lambda item: (item[0], item[1]))
    per_block: dict[int, int] = {}
    events = []
    for idx, (blk, _, user, cp, etype, amt) in enumerate(raw):
        log_idx = per_block.get(blk, 0)
        per_block[blk] = log_idx + 1
        events.append(make_event(blk, log_idx, user, cp, etype, amt, idx))
    return events


def credit_event_leaf(event: dict[str, Any]) -> str:
    payload = "|".join(str(event[f]) for f in CREDIT_HASH_FIELDS).encode("utf-8")
    return hashlib.sha256(payload).hexdigest()


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


def evaluate_credit(
    events: list[dict[str, Any]],
    user: str,
    start_block: int,
    end_block: int,
    w1: int = DEFAULT_W1,
    w2: int = DEFAULT_W2,
    alpha: int = DEFAULT_ALPHA,
    wash_window: int = DEFAULT_WASH_WINDOW,
) -> dict[str, int]:
    """Deterministic anti-wash volume + integer log2 credit score."""
    target = user.lower()
    unmatched: list[list[Any]] = []  # [block, counterparty, direction, remaining_amount]
    raw_volume = 0
    repay_count = 0
    liquidation_count = 0

    for event in events:
        b = event["block_number"]
        if b < start_block or b > end_block or event["user"].lower() != target:
            continue
        etype = event["event_type"]
        amt = int(event["amount"])
        if etype in ("SwapBuy", "SwapSell"):
            raw_volume += amt
            cp = event["counterparty"].lower()
            rem = amt
            for prev in unmatched:
                if (
                    rem > 0
                    and prev[3] > 0
                    and prev[1] == cp
                    and prev[2] != etype
                    and (b - prev[0]) <= wash_window
                ):
                    matched = min(rem, prev[3])
                    rem -= matched
                    prev[3] -= matched
            if rem > 0:
                unmatched.append([b, cp, etype, rem])
        elif etype == "Repay":
            repay_count += 1
        elif etype == "Liquidation":
            liquidation_count += 1

    clean_volume = sum(item[3] for item in unmatched)
    log_term = (1 + clean_volume).bit_length() - 1  # == Rust (1u128 + clean_volume).ilog2()
    score = min(1000, (w1 * log_term + w2 * repay_count) // (1 + alpha * liquidation_count))
    return {
        "raw_volume": raw_volume,
        "clean_volume": clean_volume,
        "wash_volume_removed": raw_volume - clean_volume,
        "repay_count": repay_count,
        "liquidation_count": liquidation_count,
        "score": score,
    }


def main() -> None:
    out_dir = Path(__file__).resolve().parent
    events = generate_credit_events()
    root = merkle_root([credit_event_leaf(e) for e in events])
    expected = {
        "dataset_id": CREDIT_DATASET_ID,
        "chain_id": CHAIN_ID,
        "protocol": CREDIT_PROTOCOL,
        "event_count": len(events),
        "merkle_root": root,
        "default_params": {
            "w1": DEFAULT_W1,
            "w2": DEFAULT_W2,
            "alpha": DEFAULT_ALPHA,
            "wash_window": DEFAULT_WASH_WINDOW,
        },
        "window": [0, DEFAULT_END_BLOCK],
        "users": {
            name: evaluate_credit(events, addr, 0, DEFAULT_END_BLOCK)
            for name, addr in USERS.items()
        },
    }
    (out_dir / "credit_events.json").write_text(
        json.dumps(events, indent=2) + "\n", encoding="utf-8"
    )
    (out_dir / "credit_expected.json").write_text(
        json.dumps(expected, indent=2) + "\n", encoding="utf-8"
    )
    print(f"wrote credit_events.json ({len(events)} events), merkle_root={root}")
    for name, m in expected["users"].items():
        print(f"  {name:5s}: clean={m['clean_volume']:>9} wash_removed={m['wash_volume_removed']:>9} score={m['score']}")


if __name__ == "__main__":
    main()