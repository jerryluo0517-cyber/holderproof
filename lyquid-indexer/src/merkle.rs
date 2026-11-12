use crate::invalid;
use holderproof_types::TransferEvent;
use lyquid::prelude::*;
use sha2::{Digest, Sha256};

/// SHA-256 → 64 lowercase hex, **no `0x` prefix**.
///
/// Prefix convention: application-level hashes (event leaves, Merkle root,
/// query hash) stay unprefixed to match the Python ground truth; only the
/// on-chain-style `block_hash` adds a `0x` prefix, via [`hash_hex_prefixed`].
pub(crate) fn hash_hex(data: &[u8]) -> String {
    use std::fmt::Write as _;
    let digest = Sha256::digest(data);
    let mut out = String::with_capacity(64);
    for byte in digest {
        write!(&mut out, "{byte:02x}").expect("writing to String cannot fail");
    }
    out
}

/// Same as [`hash_hex`], with a leading `0x` (on-chain style).
pub(crate) fn hash_hex_prefixed(data: &[u8]) -> String {
    format!("0x{}", hash_hex(data))
}

/// Serialize an event for hashing. The `|`-joined field order MUST match
/// `generate_fixture.py` (`EVENT_HASH_FIELDS` / `canonical_event`); changing
/// either side alone breaks the Rust/Python Merkle-root agreement.
pub(crate) fn event_leaf(event: &TransferEvent) -> String {
    hash_hex(
        format!(
            "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
            event.chain_id,
            event.token,
            event.block_number,
            event.block_hash,
            event.tx_hash,
            event.log_index,
            event.from,
            event.to,
            event.amount,
            event.parser_version
        )
        .as_bytes(),
    )
}

pub(crate) fn merkle_root(leaves: &[String]) -> LyquidResult<String> {
    if leaves.is_empty() {
        return Err(invalid("cannot build Merkle root from empty events"));
    }
    let mut layer = leaves.to_vec();
    while layer.len() > 1 {
        if layer.len() % 2 == 1 {
            layer.push(layer.last().expect("non-empty layer").clone());
        }
        let mut next = Vec::with_capacity(layer.len() / 2);
        for pair in layer.chunks(2) {
            let mut bytes = hex::decode(&pair[0]).map_err(|_| invalid("invalid Merkle hex"))?;
            bytes.extend(hex::decode(&pair[1]).map_err(|_| invalid("invalid Merkle hex"))?);
            next.push(hash_hex(&bytes));
        }
        layer = next;
    }
    Ok(layer.remove(0))
}
