#!/usr/bin/env python3
"""Generate a deterministic ERC-20 transfer fixture and ground-truth results.

This module deliberately uses only the Python standard library so the fixture
can be reproduced before the Lyquor/Rust environment is available.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import random
from pathlib import Path
from typing import Any, Iterable

ZERO_ADDRESS = "0x" + "00" * 20
TOKEN_ADDRESS = "0x5298000000000000000000000000000000000001"
CHAIN_ID = 11155111  # Sepolia
PARSER_VERSION = "holderproof-v1"
DEFAULT_SEED = 5298
DEFAULT_TRANSFERS = 300
DEFAULT_END_BLOCK = 1000
SNAPSHOT_BLOCKS = (900, 950, 1000)

HOLDERS = {
    "alice": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "bob": "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
    "carol": "0xcccccccccccccccccccccccccccccccccccccccc",
    "dave": "0xdddddddddddddddddddddddddddddddddddddddd",
    "erin": "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
}

Event = dict[str, Any]


def _hex_digest(*parts: object) -> str:
    raw = "|".join(str(part) for part in parts).encode("utf-8")
    return hashlib.sha256(raw).hexdigest()


def block_hash(block_number: int) -> str:
    return "0x" + _hex_digest("block", CHAIN_ID, block_number)


def tx_hash(seed: int, event_index: int) -> str:
    return "0x" + _hex_digest("tx", seed, event_index)


def make_event(
    *,
    block_number: int,
    log_index: int,
    sender: str,
    recipient: str,
    amount: int,
    seed: int,
    event_index: int,
) -> Event:
    return {
        "chain_id": CHAIN_ID,
        "token": TOKEN_ADDRESS,
        "block_number": block_number,
        "block_hash": block_hash(block_number),
        "tx_hash": tx_hash(seed, event_index),
        "log_index": log_index,
        "from": sender,
        "to": recipient,
        "amount": str(amount),
        "parser_version": PARSER_VERSION,
    }


def generate_events(
    seed: int = DEFAULT_SEED,
    transfer_count: int = DEFAULT_TRANSFERS,
    end_block: int = DEFAULT_END_BLOCK,
) -> list[Event]:
    """Generate deterministic, non-negative balances with targeted edge cases."""
    rng = random.Random(seed)
    balances: dict[str, int] = {}
    events: list[Event] = []
    event_index = 0

    # Initial mint: every holder starts with 50,000 units at block 0.
    for log_index, (name, address) in enumerate(HOLDERS.items()):
        amount = 50_000
        balances[address] = amount
        events.append(
            make_event(
                block_number=0,
                log_index=log_index,
                sender=ZERO_ADDRESS,
                recipient=address,
                amount=amount,
                seed=seed,
                event_index=event_index,
            )
        )
        event_index += 1

    rotating = list(HOLDERS.values())
    for i in range(transfer_count):
        block_number = 1 + i * 3
        if block_number >= end_block - 20:
            break

        sender = rng.choice([address for address in rotating if balances[address] > 0])
        recipient = rng.choice([address for address in rotating if address != sender])
        max_amount = min(1000, balances[sender])
        amount = rng.randint(1, max_amount)

        balances[sender] -= amount
        balances[recipient] += amount
        events.append(
            make_event(
                block_number=block_number,
                log_index=0,
                sender=sender,
                recipient=recipient,
                amount=amount,
                seed=seed,
                event_index=event_index,
            )
        )
        event_index += 1

    # Edge cases used by the Demo:
    # - Alice receives a large amount at block 950.
    # - Bob receives a large amount at block 990, just before the snapshot.
    target_events = [
        (950, ZERO_ADDRESS, HOLDERS["alice"], 10_000),
        (990, ZERO_ADDRESS, HOLDERS["bob"], 10_000),
        (999, HOLDERS["bob"], HOLDERS["alice"], 5_000),
    ]
    for block_number, sender, recipient, amount in target_events:
        events.append(
            make_event(
                block_number=block_number,
                log_index=0,
                sender=sender,
                recipient=recipient,
                amount=amount,
                seed=seed,
                event_index=event_index,
            )
        )
        event_index += 1

    events.sort(key=lambda event: (event["block_number"], event["log_index"]))
    return events


def amount_of(event: Event) -> int:
    return int(event["amount"])


def balance_at(events: Iterable[Event], holder: str, block_number: int) -> int:
    """Replay Transfer events up to and including block_number."""
    holder = holder.lower()
    balance = 0
    for event in events:
        if event["block_number"] > block_number:
            break
        if event["to"].lower() == holder:
            balance += amount_of(event)
        if event["from"].lower() == holder:
            balance -= amount_of(event)
    return balance


def twab_scaled(
    events: list[Event],
    holder: str,
    start_block: int,
    end_block: int,
    scale: int = 1_000_000,
) -> int:
    """Time-weighted average balance over [start_block, end_block), scaled."""
    if end_block <= start_block:
        raise ValueError("end_block must be greater than start_block")
    total = sum(balance_at(events, holder, block) for block in range(start_block, end_block))
    return total * scale // (end_block - start_block)


def canonical_event(event: Event) -> bytes:
    return json.dumps(event, sort_keys=True, separators=(",", ":")).encode("utf-8")


def event_leaf(event: Event) -> str:
    return hashlib.sha256(canonical_event(event)).hexdigest()


def _hash_pair(left: str, right: str) -> str:
    return hashlib.sha256(bytes.fromhex(left) + bytes.fromhex(right)).hexdigest()


def merkle_root(leaves: list[str]) -> str:
    if not leaves:
        raise ValueError("cannot build a Merkle root from an empty list")
    layer = list(leaves)
    while len(layer) > 1:
        if len(layer) % 2:
            layer.append(layer[-1])
        layer = [_hash_pair(layer[i], layer[i + 1]) for i in range(0, len(layer), 2)]
    return layer[0]


def merkle_proof(leaves: list[str], index: int) -> list[dict[str, str]]:
    if not 0 <= index < len(leaves):
        raise IndexError("leaf index out of range")
    proof: list[dict[str, str]] = []
    layer = list(leaves)
    current = index
    while len(layer) > 1:
        if len(layer) % 2:
            layer.append(layer[-1])
        sibling = current ^ 1
        proof.append({"position": "left" if sibling < current else "right", "hash": layer[sibling]})
        layer = [_hash_pair(layer[i], layer[i + 1]) for i in range(0, len(layer), 2)]
        current //= 2
    return proof


def verify_merkle_proof(leaf: str, proof: list[dict[str, str]], root: str) -> bool:
    current = leaf
    for item in proof:
        if item["position"] == "left":
            current = _hash_pair(item["hash"], current)
        else:
            current = _hash_pair(current, item["hash"])
    return current == root


def build_expected_results(events: list[Event]) -> dict[str, Any]:
    leaves = [event_leaf(event) for event in events]
    root = merkle_root(leaves)
    return {
        "seed": DEFAULT_SEED,
        "chain_id": CHAIN_ID,
        "token": TOKEN_ADDRESS,
        "event_count": len(events),
        "merkle_root": root,
        "snapshots": [
            {
                "block_number": block,
                "balances": {
                    name: balance_at(events, address, block) for name, address in HOLDERS.items()
                },
            }
            for block in SNAPSHOT_BLOCKS
        ],
        "twab_scaled_1e6": {
            name: twab_scaled(events, address, 0, DEFAULT_END_BLOCK)
            for name, address in HOLDERS.items()
        },
        "first_event_leaf": leaves[0],
        "first_event_proof": merkle_proof(leaves, 0),
        "edge_cases": {
            "unknown_holder": balance_at(events, "0x" + "99" * 20, DEFAULT_END_BLOCK),
            "block_before_any_event": balance_at(events, HOLDERS["alice"], 0),
        },
    }


def write_fixture(output_dir: Path, seed: int, transfer_count: int) -> tuple[Path, Path]:
    output_dir.mkdir(parents=True, exist_ok=True)
    events = generate_events(seed=seed, transfer_count=transfer_count)
    expected = build_expected_results(events)
    events_path = output_dir / "transfers.json"
    expected_path = output_dir / "expected_results.json"
    events_path.write_text(json.dumps(events, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    expected_path.write_text(
        json.dumps(expected, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )
    return events_path, expected_path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, default=Path(__file__).resolve().parent)
    parser.add_argument("--seed", type=int, default=DEFAULT_SEED)
    parser.add_argument("--transfers", type=int, default=DEFAULT_TRANSFERS)
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    events_path, expected_path = write_fixture(args.output_dir, args.seed, args.transfers)
    expected = json.loads(expected_path.read_text(encoding="utf-8"))
    print(f"wrote {events_path}")
    print(f"wrote {expected_path}")
    print(f"events={expected['event_count']} merkle_root={expected['merkle_root']}")


if __name__ == "__main__":
    main()
