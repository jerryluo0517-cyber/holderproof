# Step 4: Complete Query Certificate

Date: 2026-11-11  
Status: PASS

## Goal

Complete the data model, state and algorithms needed for a HolderProof query
certificate, and make the Consumer validate the full certificate.

## Rust data model

### TransferEvent

Canonical fields:

```text
chain_id, token, block_number, block_hash, tx_hash, log_index,
from, to, amount, parser_version
```

### QuerySpec

```text
chain_id, token, holder, block, parser_version
```

### Checkpoint

```text
from_block, to_block, block_hash, event_count,
event_merkle_root, parser_version
```

### QueryCertificate

```text
certificate_id, query_hash, chain_id, token, holder, block_number,
balance, checkpoint, signer_ids, status, created_at
```

## Algorithms

1. Canonical event serialization matches the Python fixture generator.
2. SHA-256 event leaves.
3. Mirrored-pair Merkle tree, matching the Python implementation.
4. Query hash from chain, token, holder, block and parser version.
5. Block hash from the deterministic fixture block hash function.
6. Balance and TWAB replay from `transfers.json`.

## Shared state

Indexer adds:

```rust
network latest_query_hash: String;
network certificates: HashMap<String, QueryCertificate>;
```

Certificates are stored by `queryHash`, not only as one latest balance.

## Consumer validation

The Consumer now calls:

```text
get_certificate_json(holder, block)
```

It verifies:

- status is `Finalized`
- at least 3 signer IDs are present
- holder and block match the request
- chain ID is Sepolia fixture chain ID
- token is present
- query hash, certificate ID and block hash are present
- event Merkle root is present
- checkpoint covers the requested block
- parser version is present

Only after these checks does it parse the balance and return eligibility.

## Tests

Rust guest tests:

```text
fixture_has_expected_event_count: PASS
alice_snapshots_match_python_ground_truth: PASS
unknown_holder_is_zero: PASS
merkle_root_matches_python_ground_truth: PASS
twab_matches_python_ground_truth: PASS
```

Cargo check:

```text
cargo check --workspace: PASS
```

## Live result

Certificate endpoint:

```text
GET http://127.0.0.1:8081/api/certificate
```

Observed certificate:

```json
{
  "certificate_id": "aebad9b697ef48fb2923895c745762167afcb88ff2c28c713a6ba358234f78ed",
  "query_hash": "aebad9b697ef48fb2923895c745762167afcb88ff2c28c713a6ba358234f78ed",
  "chain_id": 11155111,
  "token": "0x5298000000000000000000000000000000000001",
  "holder": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "block_number": 1000,
  "balance": "62685",
  "checkpoint": {
    "from_block": 0,
    "to_block": 1000,
    "block_hash": "0x1f1057f10cbab9cb537ec95ad57e4ccd5ba52ef4efd7d00dfd3aedb64e65a28e",
    "event_count": 308,
    "event_merkle_root": "2e89babbc7b45637458bd08a2d843087354bf175eb559e5c1ddc7fcc0e79617b",
    "parser_version": "holderproof-fixture-v1"
  },
  "signer_ids": [
    "Node-ve3tav5jw5ico7or34gcb6ni7rtmd35uokriuijygabd3tsltd7fj6ia",
    "Node-j3fcdy5twah7csa6pr4jhgxcvszycxdms2hsen5m7lrvtezqlv52cqaa",
    "Node-oq2lcra5u4ksufzsv2ciqbl56dkp5c653ikzkx3lh62tai566d7dseia"
  ],
  "status": "Finalized",
  "created_at": 63
}
```

Consumer endpoint:

```text
GET http://127.0.0.1:8080/api/result
```

Observed:

```json
{
  "balance": "62685",
  "eligible": true,
  "certificateStatus": "Finalized",
  "signerCount": 3,
  "queryHash": "aebad9b697ef48fb2923895c745762167afcb88ff2c28c713a6ba358234f78ed",
  "blockHash": "0x1f1057f10cbab9cb537ec95ad57e4ccd5ba52ef4efd7d00dfd3aedb64e65a28e",
  "eventMerkleRoot": "2e89babbc7b45637458bd08a2d843087354bf175eb559e5c1ddc7fcc0e79617b"
}
```

## Remaining limitations

- Data source is still the deterministic local fixture, not Sepolia live data.
- AirdropGateLyquid and GovernanceGateLyquid are not yet separate Lyquids.
- UI and E3 adversarial test suite are not yet complete.
