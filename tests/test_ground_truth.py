from __future__ import annotations

import json
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))

from fixtures.generate_fixture import (  # noqa: E402
    HOLDERS,
    balance_at,
    build_expected_results,
    event_leaf,
    generate_events,
    merkle_proof,
    merkle_root,
    verify_merkle_proof,
)


class GroundTruthTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.events = json.loads((ROOT / "fixtures" / "transfers.json").read_text(encoding="utf-8"))
        cls.expected = json.loads((ROOT / "fixtures" / "expected_results.json").read_text(encoding="utf-8"))

    def test_fixture_matches_expected_results(self) -> None:
        self.assertEqual(build_expected_results(self.events), self.expected)

    def test_event_boundary_is_inclusive(self) -> None:
        target = next(
            event
            for event in self.events
            if event["block_number"] == 950 and event["to"] == HOLDERS["alice"]
        )
        self.assertGreater(
            balance_at(self.events, HOLDERS["alice"], target["block_number"]),
            balance_at(self.events, HOLDERS["alice"], target["block_number"] - 1),
        )

    def test_unknown_holder_has_zero_balance(self) -> None:
        self.assertEqual(balance_at(self.events, "0x" + "99" * 20, 1000), 0)

    def test_merkle_proof_round_trip(self) -> None:
        leaves = [event_leaf(event) for event in self.events]
        root = merkle_root(leaves)
        for index in (0, len(leaves) // 2, len(leaves) - 1):
            proof = merkle_proof(leaves, index)
            self.assertTrue(verify_merkle_proof(leaves[index], proof, root))

    def test_tampered_event_fails_root(self) -> None:
        original_root = self.expected["merkle_root"]
        tampered = json.loads(json.dumps(self.events))
        tampered[0]["amount"] = str(int(tampered[0]["amount"]) + 1)
        self.assertNotEqual(merkle_root([event_leaf(event) for event in tampered]), original_root)

    def test_generation_is_deterministic(self) -> None:
        self.assertEqual(generate_events(), self.events)


if __name__ == "__main__":
    unittest.main()
