# Certificate issuance and validation

Four committee members replay the input history. Certification requires at
least three members to agree on the full fact. This page describes balance
certificates. The [course report](../reports/HolderProof_Academic_Report_Cover_Updated.pdf)
covers the credit, epoch, and Sepolia paths alongside fixture balances.

## From query to certificate

1. Each member replays the requested history and builds a `BalanceFact`.
2. The Indexer groups identical facts and counts each participating member once.
   It excludes members that submit duplicate proposals.
3. Lyquor signs and authenticates the call that carries the agreed fact.
4. The Indexer replays the fact, resolves the signers, and stores a
   `QueryCertificate` with the original `OracleCert` and signature bytes.
5. Repeating a query returns its original certificate. The deploying
   initializer can mark the certificate invalid.

## What each gate checks

| Check | Rule |
|---|---|
| Status and query | The balance certificate is `Finalized`. The holder and block match the request, and the chain, token and parser pass the dataset rules. The query hash and certificate ID match the recomputed ID. |
| Fixture checkpoint | The range starts at block 0 and ends at the requested block. The endpoint hashes match the fixture, and the checkpoint carries an event count and a Merkle root. |
| Sepolia checkpoint | The snapshot range ends at the requested block. The boundary hashes and Merkle root have the expected encoding, and the event count is positive. The replay uses the embedded WETH snapshot. |
| Fact digest | The digest covers the query type, balance, and all checkpoint fields. |
| Signer evidence | Three or four distinct node IDs are paired with distinct native signers and their 64-byte approval signatures. |
| Source | The target Lyquid, sequence backend, and state reference match the configured Indexer. |
| Eligibility | The gate parses the balance as `U256` and compares it with its own threshold. |

The deploying initializer configures each gate with nonzero addresses. When the
policy changes, the gate clears its last decision. It loads the current
certificate on every eligibility check and before recording an action.
`claim()` records one entitlement per holder. `vote()` records one vote per
proposal and holder. Both match the holder to the caller.

## State and trust

The configured Indexer supplies the certificate and Lyquor authenticates the
certified write. Gates check the attached runtime evidence for consistency and
verify its source. The committee assumes access to complete Transfer history.

Shared facts and gate policy use network state, where LDK 0.8.4 supports atomic
`call!`. Instance state stores each node's replay cache and recent gate
decisions. Read the latter at `/api/history`.

Fixture block hashes are deterministic synthetic values. The archival EVM check
compares replay balances with historical `balanceOf` on a separate local test
token, using the documented block offset.
[03/04 acceptance and historical comparison](../reports/PLAN_03_04_ACCEPTANCE.md)
