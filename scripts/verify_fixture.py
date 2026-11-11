#!/usr/bin/env python3
"""Verify the committed fixture against its expected ground-truth results."""

from __future__ import annotations

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))

from fixtures.generate_fixture import (  # noqa: E402
    HOLDERS,
    balance_at,
    build_expected_results,
    event_leaf,
    merkle_root,
    verify_merkle_proof,
)


def load_json(path: Path):
    return json.loads(path.read_text(encoding="utf-8"))


def main() -> int:
    events = load_json(ROOT / "fixtures" / "transfers.json")
    expected = load_json(ROOT / "fixtures" / "expected_results.json")
    actual = build_expected_results(events)

    checks: list[tuple[str, bool]] = [
        ("event count", actual["event_count"] == expected["event_count"]),
        ("Merkle root", actual["merkle_root"] == expected["merkle_root"]),
        ("snapshot balances", actual["snapshots"] == expected["snapshots"]),
        ("TWAB", actual["twab_scaled_1e6"] == expected["twab_scaled_1e6"]),
        ("unknown holder is zero", balance_at(events, "0x" + "99" * 20, 1000) == 0),
        (
            "first event Merkle proof",
            verify_merkle_proof(
                event_leaf(events[0]),
                expected["first_event_proof"],
                expected["merkle_root"],
            ),
        ),
    ]

    # A single-event mutation must change the event root.
    tampered = json.loads(json.dumps(events))
    tampered[0]["amount"] = str(int(tampered[0]["amount"]) + 1)
    checks.append(("tamper detection", merkle_root([event_leaf(e) for e in tampered]) != expected["merkle_root"]))

    # Confirm the expected snapshot includes Bob's targeted late transfer.
    bob = expected["snapshots"][-1]["balances"]["bob"]
    alice = expected["snapshots"][-1]["balances"]["alice"]
    checks.append(("demo holders exist", bob > 0 and alice > 0))
    checks.append(("known addresses", all(addr.startswith("0x") for addr in HOLDERS.values())))

    failures = [name for name, ok in checks if not ok]
    for name, ok in checks:
        print(f"[{'PASS' if ok else 'FAIL'}] {name}")

    if failures:
        print(f"\n{len(failures)} check(s) failed: {', '.join(failures)}")
        return 1
    print(f"\nAll {len(checks)} checks passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
