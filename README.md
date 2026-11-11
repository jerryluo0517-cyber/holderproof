# HolderProof

**Verifiable ERC-20 historical holder proof for retrospective airdrops.**

HolderProof recomputes an address's historical ERC-20 balance from `Transfer`
events and turns the result into a committee-certified Lyquor fact that a
consumer Lyquid can use for airdrop or governance gating.

## Documentation

- [Scenario 1 execution plan](docs/HolderProof_Scenario1_Plan.pdf)
- [Step 2–4 operation log](docs/HolderProof_Step2-4_Operation_Log.pdf)
- [Collaboration guide](COLLABORATOR_GUIDE.md)
- [Step 2 certified-call record](docs/step2-certified-call.md)
- [Step 3 fixture replay record](docs/step3-fixture-replay.md)

## Status: Step 4 complete (2026-11-11)

Verified environment:

- Rust 1.99.0 + `wasm32-unknown-unknown`
- Foundry 1.7.1
- Lyquor LDK 0.8.4
- local 4-node Lyquor devnet
- official `hello` and `price-feed` examples

Current working flow:

```text
ERC-20 Transfer fixture
  -> Rust balance replay
  -> 4-node committee proposal
  -> 3-of-4 certified update
  -> full QueryCertificate
  -> Consumer call! and certificate validation
  -> eligible / not eligible
```

Current fixtures:

- 308 deterministic `Transfer` events
- Alice at block 900 = 47685
- Alice at block 950 = 57685
- Alice at block 1000 = 62685
- Bob at block 1000 = 51886
- all certified results use 3 committee signers

The current data source is a **local deterministic fixture**, not Sepolia live
data. The Lyquor committee execution and cross-Lyquid call are real within the
local devnet. Certificates bind the query hash, block hash, event Merkle root,
parser version, signer IDs and finality status.

## Repository layout

```text
lyquid-indexer/       historical balance indexer and committee flow
lyquid-consumer/      consumer using call! against the certified balance
fixtures/             deterministic Transfer fixture and expected results
scripts/              devnet and fixture verification scripts
tests/                Python ground-truth tests
docs/                 architecture, operation logs and reports
```

## Requirements

- Rust 1.99.0
- Lyquor LDK 0.8.4
- Foundry
- Python 3.12+

## Commands

```bash
# Regenerate and verify the fixture
python3 fixtures/generate_fixture.py
python3 scripts/verify_fixture.py

# Run Python ground-truth tests
python3 -m unittest discover -s tests -v

# Compile both Lyquids
cargo check --workspace

# Run the complete local check suite
make check

# Run Lyquid guest tests
shaker test lyquid-indexer/Cargo.toml
shaker test lyquid-consumer/Cargo.toml

# Start the four-node devnet
./scripts/start-devnet.sh multi
```

## Core invariant

```text
sum(inbound amounts) - sum(outbound amounts)
= balance(holder, block_number)
```

The Rust Indexer result is checked against the independent Python fixture
answer. Both data sources must pass through the same replay, committee,
certificate and consumer path.

## Scope

P0:

- one standard ERC-20 token
- one holder and one historical block per query
- four-node committee with threshold `3`
- certified balance reads through `call!`
- consumer eligibility result

Not yet implemented:

- AirdropGateLyquid / GovernanceGateLyquid as separate Lyquids
- Sepolia live data adapter
- UI
- E3 adversarial tests
- final report and recorded demo

## Next steps

1. Step 5: AirdropGateLyquid and GovernanceGateLyquid.
2. Step 6: E3 tamper, missing-signature and wrong-block tests.
3. Step 7: minimal UI and user testing.
4. Step 8: report, slides and recorded demo.
