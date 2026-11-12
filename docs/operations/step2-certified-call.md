# Step 2: Certified Call and Cross-Lyquid Read

## Goal

Prove the P0 architectural gate:

```text
4-node committee
  -> 3-of-4 certified balance fact
  -> shared Lyquor network state
  -> consumer Lyquid call!
  -> eligible / not eligible
```

## Initial files

| File | Purpose |
|---|---|
| `lyquid-indexer/src/lib.rs` | Stores the historical balance fact and exposes `get_balance` |
| `lyquid-consumer/src/lib.rs` | Entry points and state |
| `lyquid-consumer/src/certificate.rs` | Certificate trust rules |
| `lyquid-consumer/src/eligibility.rs` | Balance parsing and threshold logic |
| `lyquid-consumer/src/http_endpoints.rs` | Result diagnostics |
| `lyquid-consumer/src/tests.rs` | Certificate validation tests |
| `scripts/start-devnet.sh` | Starts the official 4-node devnet |
| `docs/operations/step2-certified-call.md` | Records the integration gate |

## Implementation order

1. Compile both minimal Lyquids.
2. Start the 4-node devnet.
3. Deploy the indexer and record its Lyquid ID.
4. Deploy the consumer and configure it with the indexer ID, block, and threshold.
5. Publish one known balance fact.
6. Call `is_eligible` and verify the result.
7. Replace the plain `publish_balance` write with the oracle committee pattern from
   `price-feed`: `network oracle`, `configure_committee`, `propose_and_certify`, and
   `submit_certified_call!`.
8. Confirm 3-of-4 agreement and rejection when fewer than three nodes agree.

## Current scope

The first slice intentionally keeps `publish_balance` as a plain network write. It is a
compile/deploy/call gate, not the final trust model.

---

## Execution result (2026-11-11)

**Status: PASS**

### Deployed Lyquids

| Role | Lyquid ID | Backend contract |
|---|---|---|
| Indexer | `Lyquid-ahjg4huwvban3bvhxue7rcbaw4mnltwyolhqa` | `0x59b670e9fA9D0A427751Af201D676719a970857b` |
| Consumer | `Lyquid-wsd6v4xjliqgjrcaiikuafrjmaho4q4hqiwqa` | `0x84eA74d481Ee0A5332c457a4d796187F6Ba67fEB` |

### Committee certification

- 4 nodes configured
- Threshold: `3`
- Topic: `balance_proof`
- Oracle epoch: `1`
- `report_balance()` returned `true`
- Certified balance: `62685`
- Certified signer count: `3`

Indexer diagnostic endpoint:

```text
GET http://127.0.0.1:8081/api/certified
```

Observed:

```json
{"holder":"0xaAaAaAaaAaAaAaaAaAAAAAAAAaaaAaAaAaaAaaAa","block":1000,"balance":"62685","signerCount":3}
```

### Consumer cross-Lyquid read

Consumer configuration:

```text
indexer:  0x01D26E1e96A840dD86A7BD09F88820b718D5CEd8
holder:   0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
block:    1000
threshold: 60000
```

Consumer diagnostic endpoint:

```text
GET http://127.0.0.1:8080/api/result
```

Observed:

```json
{"balance":"62685","block":1000,"checkCount":1,"eligible":true,"holder":"0xaAaAaAaaAaAaAaaAaAAAAAAAAaaaAaAaAaaAaaAa","indexer":"0x01D26E1e96A840dD86A7BD09F88820b718D5CEd8","threshold":"60000"}
```

### Environment note

- Network writes were submitted through the shared Anvil backend.
- Oracle activation/finalization used the Lyquor node WebSocket endpoint.
- The final Consumer read uses `call!` -> `get_certified_balance`, not the plain snapshot.
- The committee currently proposes a deterministic test balance. The next milestone replaces it with real fixture replay.
