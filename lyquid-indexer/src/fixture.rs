use crate::invalid;
use crate::merkle::{event_leaf, hash_hex, hash_hex_prefixed, merkle_root};
use holderproof_types::{CHAIN_ID, Checkpoint, PARSER_VERSION, QuerySpec, TransferEvent};
use lyquid::prelude::*;
use std::sync::LazyLock;

const FIXTURE_JSON: &str = include_str!("../../fixtures/transfers.json");

/// Parse and sort the embedded fixture once, then reuse it for every query.
/// Errors are cached as a `String` so the `LazyLock` stays `Sync`.
static FIXTURE_EVENTS: LazyLock<Result<Vec<TransferEvent>, String>> = LazyLock::new(|| {
    let mut events: Vec<TransferEvent> =
        serde_json::from_str(FIXTURE_JSON).map_err(|err| format!("fixture JSON error: {err}"))?;
    events.sort_by_key(|event| (event.block_number, event.log_index));
    Ok(events)
});

/// Normalize an `Address` to its lowercase `0x…` hex string. Fixture event
/// `from`/`to` fields are already lowercase, so string comparison is safe as
/// long as both sides are normalized here — never inline `to_ascii_lowercase`.
fn addr_lowercase(addr: Address) -> String {
    addr.to_string().to_ascii_lowercase()
}

/// Compare a query `Address` against a lowercase hex string field.
fn addr_eq(addr: Address, field: &str) -> bool {
    addr_lowercase(addr) == field.to_ascii_lowercase()
}

/// The fixture events, already sorted by `(block_number, log_index)`.
pub(crate) fn fixture_events() -> LyquidResult<&'static [TransferEvent]> {
    match &*FIXTURE_EVENTS {
        Ok(events) => Ok(events.as_slice()),
        Err(err) => Err(invalid(err.as_str())),
    }
}

pub(crate) fn fixture_token() -> LyquidResult<String> {
    fixture_events()?
        .first()
        .map(|event| event.token.clone())
        .ok_or_else(|| invalid("fixture has no events"))
}

pub(crate) fn fixture_balance(holder: Address, block: u64) -> LyquidResult<U256> {
    let mut balance = U256::ZERO;
    for event in fixture_events()? {
        if event.block_number > block {
            break;
        }
        let amount = event
            .amount
            .parse::<U256>()
            .map_err(|_| invalid(format!("invalid fixture amount: {}", event.amount)))?;
        if addr_eq(holder, &event.to) {
            balance += amount;
        }
        if addr_eq(holder, &event.from) {
            balance -= amount;
        }
    }
    Ok(balance)
}

pub(crate) fn fixture_twab(
    holder: Address,
    start: u64,
    end: u64,
    scale: u64,
) -> LyquidResult<U256> {
    if end <= start {
        return Err(invalid("TWAB end block must be greater than start block"));
    }
    let events = fixture_events()?;
    let mut current = U256::ZERO;
    let mut total = U256::ZERO;
    let mut index = 0usize;
    for block_number in start..end {
        while index < events.len() && events[index].block_number <= block_number {
            let event = &events[index];
            let amount = event
                .amount
                .parse::<U256>()
                .map_err(|_| invalid(format!("invalid fixture amount: {}", event.amount)))?;
            if addr_eq(holder, &event.to) {
                current += amount;
            }
            if addr_eq(holder, &event.from) {
                current -= amount;
            }
            index += 1;
        }
        total += current;
    }
    Ok(total * U256::from(scale) / U256::from(end - start))
}

pub(crate) fn fixture_block_hash(block: u64) -> String {
    hash_hex_prefixed(format!("block|{CHAIN_ID}|{block}").as_bytes())
}

pub(crate) fn fixture_checkpoint(block: u64) -> LyquidResult<Checkpoint> {
    let events: Vec<TransferEvent> = fixture_events()?
        .iter()
        .filter(|event| event.block_number <= block)
        .cloned()
        .collect();
    let leaves: Vec<String> = events.iter().map(event_leaf).collect();
    Ok(Checkpoint {
        from_block: 0,
        to_block: block,
        block_hash: fixture_block_hash(block),
        event_count: events.len() as u64,
        event_merkle_root: merkle_root(&leaves)?,
        parser_version: PARSER_VERSION.to_string(),
    })
}

pub(crate) fn make_query_spec(token: &str, holder: Address, block: u64) -> QuerySpec {
    QuerySpec {
        chain_id: CHAIN_ID,
        token: token.to_string(),
        holder: addr_lowercase(holder),
        block,
        parser_version: PARSER_VERSION.to_string(),
    }
}

/// Bind chain/token/holder/block/parser into the certificate id. Rust-only
/// (the Python ground truth has no query hash), but keep the field order
/// stable since it is the certificate lookup key.
pub(crate) fn query_hash(spec: &QuerySpec) -> String {
    hash_hex(
        format!(
            "query|{}|{}|{}|{}|{}",
            spec.chain_id, spec.token, spec.holder, spec.block, spec.parser_version
        )
        .as_bytes(),
    )
}
