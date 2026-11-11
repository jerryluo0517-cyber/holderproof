# HolderProof

**Verifiable ERC-20 historical holder proof for retrospective airdrops.**

HolderProof recomputes an address's historical ERC-20 balance from `Transfer`
events and turns the result into a certificate that a downstream Lyquid can
consume for airdrop/gating decisions.

## Documentation

- [Scenario 1 execution plan](docs/HolderProof_Scenario1_Plan.pdf)
- [Step 2–3 operation log](docs/HolderProof_Step2-3_Operation_Log.pdf)
- [Collaboration guide](COLLABORATOR_GUIDE.md)

## Environment verified (2026-11-11)

- Rust 1.99.0 + wasm32 target
- Foundry 1.7.1
- Lyquor LDK 0.8.4
- single-node native devnet started successfully
- official `hello` Lyquid deployed successfully
- official `price-feed` example identified as the committee/certified-call reference

Platform adjustment: the native multi-node devnet has 4 nodes. HolderProof will use a
**3-of-4** committee threshold instead of the earlier 2-of-3 assumption.

See `docs/lyquor-step1.md` for commands, IDs, confirmed APIs, and the next integration gate.

## Step 2 verified (2026-11-11)

- Four-node localnet started successfully.
- Indexer and Consumer Lyquids deployed and updated.
- Four-node committee configured with threshold `3`.
- `report_balance()` produced a certified balance with `3` signers.
- Consumer used `call!` to read the certified balance instead of the plain snapshot.
- Consumer returned `eligible=true` for balance `62685` against threshold `60000`.

See `docs/step2-certified-call.md` for IDs, contracts, diagnostics, and environment notes.

## Current implementation status

This repository currently contains WP1 and the independent ground-truth layer:

- deterministic ERC-20 `Transfer` fixture
- exact balance replay at selected historical blocks
- time-weighted average balance (TWAB) ground truth
- canonical event hashing and Merkle root/proofs
- tamper-detection tests

The Lyquor/Rust integration is the next layer. Keeping the Python ground truth
independent makes it useful for cross-checking the Rust implementation.

## Scope freeze (P0)

- one standard ERC-20 token
- one holder and one historical block per query
- `balance(holder, blockN)`
- 3 nodes, 2/3 agreement
- certificate includes block hash, event Merkle root and node signatures
- `AirdropGateLyquid` returns eligible / not eligible
- local fixture is the primary reproducible demo; Sepolia is optional

Out of scope for P0: generic SQL, arbitrary ABI parsing, DeFi credit scoring,
price/P&L, ZK proofs, cross-chain, mainnet-scale indexing.

## Commands

```bash
# Regenerate fixtures
python3 fixtures/generate_fixture.py

# Verify the fixture and Merkle proofs
python3 scripts/verify_fixture.py

# Run the standard-library test suite
python3 -m unittest discover -s tests -v
```

Expected result:

```text
All checks passed.
```

## Fixture layout

- `fixtures/transfers.json`: canonical ERC-20 Transfer events
- `fixtures/expected_results.json`: balances, TWAB, Merkle root and proof
- `fixtures/generate_fixture.py`: deterministic generator and algorithms
- `scripts/verify_fixture.py`: ground-truth verifier
- `tests/test_ground_truth.py`: correctness and tamper tests

## Core invariant

```text
sum(inbound amounts) - sum(outbound amounts)
= balance_at(holder, block_number)
```

The reconstructed balance must match an independent Python replay and, in the
Sepolia integration, the archive node's historical `balanceOf(holder)` at the
same block.

## Next implementation steps

1. Port `event_leaf`, `balance_at` and Merkle verification to Rust.
2. Run the same fixture through one Lyquid node and compare with
   `expected_results.json`.
3. Extend to 3 nodes with a 2/3 signed quorum.
4. Add certificate validation and `AirdropGateLyquid`.
5. Add a one-command end-to-end demo and README reproduction script.
