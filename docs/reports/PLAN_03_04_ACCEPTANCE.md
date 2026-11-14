# Execution plan 03/04: P0 acceptance

Date: 2026-11-15. Baseline: Scenario 1 execution plan v1.1, sections 03 and 04.
The original plan remains unchanged. This record separates its requirements
from native LDK representations and the scope of the automated evidence.

## 03: models, state and algorithms

| Requirement | Implementation / evidence |
|---|---|
| Deterministic event leaves | Ordered `(block_number, log_index)`; Python/Rust Merkle agreement; source metadata and duplicate-position checks |
| Checkpoint | Dataset ID, genesis/end block hashes, range, event count, root, parser version |
| Historical balance | Checked U256 replay; fixture snapshot tests; independent Python answers |
| Historical `balanceOf` comparison | 308 actual EVM logs; 18 archival calls, covering five holders and an unknown holder at three snapshots; certificate balance comparison |
| Query certificate | Query type, query/fact hashes, checkpoint, balance, native signature bytes, signer mapping, network state reference, status and creation position |
| Certificate status | Finalized on certified commit; initializer can invalidate; every gate checks current status on each action |
| Network state | Canonical certificates, committee configuration, selected query, gate policy and replay-protected action records |
| Instance / private state | Per-node proposal cache; bounded gate query/result history; node-local embedded fixture and parsed event cache |
| P1 | TWAB is implemented using event intervals; MIN_BALANCE, ACTIVE_RATIO and window certificates remain optional work |

Archival comparison uses an isolated test-only Solidity ledger. Deployment is
EVM block 1, so fixture N maps to EVM N+2. Every Transfer's sender, recipient and
amount is checked against its receipt logs, and historical state is read after
all transfers are replayed. This is a local EVM correctness cross-check, not an
archive Sepolia/RPC integration and not proof of the synthetic fixture's hashes.

## 04: implementation scope and delivery interfaces

| P0 item | Implementation / evidence |
|---|---|
| Devnet | Official repeatable four-node LDK infrastructure; business committee uses exactly three distinct members |
| Fixture | 308 sorted, reproducible events and independent answers |
| Single-node replay | Python/Rust snapshots and Merkle tests |
| Three-node execution / 2-of-3 | Three member IDs, threshold two; full fact equality; duplicate node exclusion |
| Shared certificate | Certified call carries full fact JSON, including checkpoint; original native certificate and signature bytes persisted |
| AirdropGateLyquid | Separate deployment from `lyquid-consumer`; accepts 60000 / rejects 70000 against Alice's balance 62685 |
| GovernanceGateLyquid | Separate crate/deployment; accepts 50000 / rejects 70000 using the same certificate |
| README / CLI / reproduction | `run-proof.sh --token --holder --block`; `make acceptance`; saved JSON evidence |

Both gate policies are exercised in the real local network. The certificate
remains byte-for-byte unchanged across consumer use and repeated query lookup.
The original `lyquid-consumer` path is retained as the airdrop implementation;
it is not a third unnamed substitute for a missing consumer.

## Deliberate representations and adaptations

- A single historical query has `query_type = Balance`, `block_number = N`,
  and checkpoint range `0..N`. These encode the plan's `toBlock`/query window.
- A certificate's `balance` is the plan's `resultValue`; checkpoint fields are
  nested rather than duplicated at the top level.
- Threshold is gate policy, not a property of the shared balance fact. Keeping
  it outside the certificate lets both consumers reuse one immutable fact.
- `node_signatures` stores actual native signature bytes with signer IDs;
  `runtime_certificate` preserves the original native header and signatures.
  LDK authenticates them on certified submission. JSON consistency checking
  is not independent detached-signature verification.
- `network_state_id` is `LyquidID@creation_block`; the native header also records
  sequence backend, epoch, config hash and nonce. This is provenance, not a
  Merkle inclusion proof of network state.
- Atomic cross-Lyquid `call!` is network-only in LDK 0.8.4. Shared policy and
  committed action results therefore stay in network state. Instance state
  stores local replay caches and bounded query/result histories. Moving the
  authoritative gate policy only to instance state would break deterministic
  cross-Lyquid composition.
- Four localnet infrastructure nodes do not imply four committee members.
  The application policy is exactly two approvals from three members.

The plan's claim that one erroneous node always prevents a 2-of-3 quorum is
inconsistent with that threshold. Two matching honest members can still form a
quorum; one bad vote alone, duplicate votes, or three different facts cannot.
The tests use this quorum interpretation.

## Reproduce and inspect evidence

```bash
make acceptance
```

Outputs:

- `.run/step4/certificate.json`: full native evidence and certified fact.
- `.run/step4/gate-cases.json`: both outcomes from both consumers.
- `.run/acceptance/historical-balances.json`: block mapping and archival balances.
- `.run/step4/localnet.log`: deployment and native execution log.

Guest tests use synthetic evidence only when testing JSON structure. The real
network demo supplies the cryptographically authenticated native certificates.
Eligibility demos exercise both gate decisions; they do not constitute external
user tests or a complete economic/governance application.

## Verified result (2026-11-15)

`make acceptance` completed successfully on the final code:

| Check | Result |
|---|---|
| Workspace compilation | PASS, all five crates |
| Indexer guest tests | 12 passed |
| Shared gate guest tests | 12 passed |
| Python ground-truth tests | 6 passed |
| Fixture checks | 9 passed |
| Native 2-of-3 certificate | PASS; actual signature evidence retained |
| Both gates, both eligibility outcomes | PASS |
| Repeated query / unchanged certificate | PASS |
| Per-instance result history | PASS |
| EVM Transfer receipts / archival balances | 308 logs / 18 queries passed |
| Rust format, shell/Python syntax, diff whitespace | PASS |

The complete run is recorded in `.run/acceptance/final-check.log`. Claim/vote
entry points are implemented, but the automated end-to-end evidence above
covers eligibility and certificate reuse rather than a full payout/voting flow.

## Remaining boundaries

P0 is restricted to the fixture's one standard token and finalized block range
0..1000. Synthetic fixture block hashes are not live chain finality evidence.
Sepolia ingestion, independent RPC diversity, an external signature-verifier
registry, full E3 fault-injection/user testing, a UI, report and recorded demo
are not claimed by this acceptance record. Existing dated PDFs remain historical.
