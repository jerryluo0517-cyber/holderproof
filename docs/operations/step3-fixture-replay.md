# Step 3: Real ERC-20 Transfer Fixture Replay

Date: 2026-11-11  
Status: PASS

## Goal

Replace the deterministic committee proposal constant with a real replay of
`fixtures/transfers.json`.

## Implementation

### Embedded fixture

The Indexer embeds the fixture at compile time:

```rust
const FIXTURE_JSON: &str = include_str!("../../fixtures/transfers.json");
```

No external filesystem or RPC dependency is required in the Lyquid runtime.

### Rust replay

The Indexer parses and sorts events by `(block_number, log_index)`, then applies:

```text
to == holder   -> balance += amount
from == holder -> balance -= amount
```

Only events with `block_number <= query_block` are included.

### Query configuration

New network state:

```text
query_holder: Address
query_block: u64
```

New network function:

```text
set_query(holder, block)
```

The committee's `propose()` reads this query and independently computes the
same fixture balance on every node.

## Tests

Rust guest tests:

```text
fixture_has_expected_event_count: PASS
alice_snapshots_match_python_ground_truth: PASS
unknown_holder_is_zero: PASS
```

Cargo check:

```text
cargo check --workspace: PASS
```

## Live verification

### Alice

Query:

```text
holder = 0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
block  = 1000
```

Certified result:

```json
{
  "holder": "0xaAaAaAaaAaAaAaaAaAAAAAAAAaaaAaAaAaaAaaAa",
  "block": 1000,
  "balance": "62685",
  "signerCount": 3
}
```

### Bob

Query:

```text
holder = 0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
block  = 1000
```

Certified result:

```json
{
  "holder": "0xbBbBBBBbbBBBbbbBbbBbbbbBBbBbbbbBbBbbBBbB",
  "block": 1000,
  "balance": "51886",
  "signerCount": 3
}
```

This proves the committee is replaying fixture data rather than returning a
fixed value.

### Consumer final result

Consumer output after restoring Alice's query:

```json
{
  "balance": "62685",
  "block": 1000,
  "checkCount": 3,
  "eligible": true,
  "holder": "0xaAaAaAaaAaAaAaaAaAAAAAAAAaaaAaAaAaaAaaAa",
  "indexer": "0x01D26E1e96A840dD86A7BD09F88820b718D5CEd8",
  "threshold": "60000"
}
```

## Step 4 extension

The full event Merkle root, block hash, query hash, parser version and signer
IDs are now included by the Step 4 certificate implementation.
