# Full-fact certificate and consumer validation

Updated: 2026-11-15. For execution-plan section mapping and acceptance evidence,
see [03/04 acceptance](../reports/PLAN_03_04_ACCEPTANCE.md).

## Certification path

1. Each of three committee nodes replays the fixture and builds `BalanceFact`.
2. A fact contains the complete query, query type, balance and checkpoint.
3. Aggregation requires two identical facts from distinct nodes; a node that
   submits repeatedly is excluded from the candidate vote count.
4. The returned certified-call input contains the entire serialized fact.
5. Lyquor's native certification signs that payload and authenticates submission.
6. The Indexer defensively replays the fact, resolves signer IDs, retains actual
   signature bytes and the native OracleCert, and stores a QueryCertificate.
7. Identical queries preserve the original certificate, signatures and creation
   position. An initializer-only method can invalidate its current status.

The Indexer also keeps a per-node instance cache of replayed proposals.

## Shared consumer rules

`holderproof-gate-core` checks:

- current certificate status is Finalized;
- two or three distinct, nonempty node IDs;
- native signer/signature counts, IDs and byte values match exported evidence;
- Ed25519 approval signatures have 64 bytes;
- source Lyquid, sequence backend and state reference match the configured Indexer;
- chain, token, holder, block and parser match the supported query;
- query hash and certificate ID match recomputed query identity;
- full fact hash matches query, type, balance and every checkpoint field;
- fixture dataset ID and both endpoint block hashes match the deterministic source;
- checkpoint starts at genesis, ends at the requested finalized fixture block,
  and contains a nonzero event count and correctly encoded Merkle digest;
- balance parses as U256 before comparison with each gate's own threshold.

Both gates restrict configuration to the deploying initializer, reject zero
addresses and clear the last decision when policy changes. They fetch and
validate the current certificate on each eligibility/action request.
`claim()` binds a one-time entitlement to the caller; `vote()` binds one vote
per proposal/holder to the caller. They are qualification records, not payouts
or a complete DAO execution system.

## State split

Canonical facts and gate policy use network state. The native LDK `call!` API
requires network execution, so moving authoritative gate policy/results solely
to instance state would not preserve deterministic composition. Bounded local
query/result histories use instance state and are available at `/api/history`.
The local fixture and replay caches are per-node data.

## Trust boundary

Native certified-write authentication and the initializer-configured Indexer
are the trust anchors. The exported native signature evidence is auditable;
consumers do not independently verify detached signatures against a separate
committee-key registry. A self-consistent fabricated JSON object is not a valid
substitute for an authenticated Indexer call. Fact digests detect inconsistent
payloads, but do not provide mathematical proof that a data source is truthful.

Fixture hashes are synthetic, not Sepolia headers. The independent archival
EVM check verifies replay arithmetic against actual token state with a documented
block offset; it does not validate Sepolia availability, reorgs or live finality.
