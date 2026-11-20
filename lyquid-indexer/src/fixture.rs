use crate::invalid;
use crate::merkle::{event_leaf, merkle_root};
use holderproof_types::{CHAIN_ID, Checkpoint, PARSER_VERSION, QuerySpec, TransferEvent};
use lyquid::prelude::*;
use std::sync::LazyLock;

const FIXTURE_JSON: &str = include_str!("../../fixtures/transfers.json");

/// Parse and sort the embedded fixture once, then reuse it for every query.
/// Errors are cached as a `String` so the `LazyLock` stays `Sync`.
static FIXTURE_EVENTS: LazyLock<Result<Vec<TransferEvent>, String>> =
    LazyLock::new(|| parse_events(FIXTURE_JSON));

pub(crate) fn parse_events(json: &str) -> Result<Vec<TransferEvent>, String> {
    let mut events: Vec<TransferEvent> =
        serde_json::from_str(json).map_err(|err| format!("fixture JSON error: {err}"))?;
    events.sort_by_key(|event| (event.block_number, event.log_index));
    for (index, event) in events.iter().enumerate() {
        if event.chain_id != CHAIN_ID
            || event.token != holderproof_types::FIXTURE_TOKEN
            || event.parser_version != PARSER_VERSION
            || event.block_hash != holderproof_types::fixture_block_hash(event.block_number)
            || event.block_number > holderproof_types::FIXTURE_END_BLOCK
        {
            return Err(format!("invalid fixture source metadata at event {index}"));
        }
        if index > 0
            && events[index - 1].block_number == event.block_number
            && events[index - 1].log_index == event.log_index
        {
            return Err(format!("duplicate event position at event {index}"));
        }
        if event.from.parse::<Address>().is_err()
            || event.to.parse::<Address>().is_err()
            || event.amount.parse::<U256>().is_err()
        {
            return Err(format!("invalid address or amount at event {index}"));
        }
    }
    Ok(events)
}

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
    replay_balance(fixture_events()?, holder, block)
}

/// Replay validated events in ascending block and log index order.
pub(crate) fn replay_balance(
    events: &[TransferEvent],
    holder: Address,
    block: u64,
) -> LyquidResult<U256> {
    let mut balance = U256::ZERO;
    for event in events {
        if event.block_number > block {
            break;
        }
        balance = apply_transfer(balance, holder, event)?;
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
    let mut current = fixture_balance(holder, start)?;
    let mut total = U256::ZERO;
    let mut cursor = start;
    for event in events
        .iter()
        .filter(|event| event.block_number > start && event.block_number < end)
    {
        let duration = U256::from(event.block_number - cursor);
        let area = current
            .checked_mul(duration)
            .ok_or_else(|| invalid("TWAB product overflow"))?;
        total = total
            .checked_add(area)
            .ok_or_else(|| invalid("TWAB sum overflow"))?;
        current = apply_transfer(current, holder, event)?;
        cursor = event.block_number;
    }
    let tail = current
        .checked_mul(U256::from(end - cursor))
        .ok_or_else(|| invalid("TWAB product overflow"))?;
    total
        .checked_add(tail)
        .and_then(|total| total.checked_mul(U256::from(scale)))
        .map(|total| total / U256::from(end - start))
        .ok_or_else(|| invalid("TWAB scaled total overflow"))
}

fn apply_transfer(balance: U256, holder: Address, event: &TransferEvent) -> LyquidResult<U256> {
    let amount = event
        .amount
        .parse::<U256>()
        .map_err(|_| invalid("invalid fixture amount"))?;
    let incoming = addr_eq(holder, &event.to);
    let outgoing = addr_eq(holder, &event.from);
    match (incoming, outgoing) {
        (true, false) => balance
            .checked_add(amount)
            .ok_or_else(|| invalid("balance overflow")),
        (false, true) => balance
            .checked_sub(amount)
            .ok_or_else(|| invalid("negative replay balance")),
        _ => Ok(balance), // Self transfers do not change the holder's balance.
    }
}

pub(crate) fn fixture_block_hash(block: u64) -> String {
    holderproof_types::fixture_block_hash(block)
}

pub(crate) fn fixture_checkpoint(block: u64) -> LyquidResult<Checkpoint> {
    let events: Vec<TransferEvent> = fixture_events()?
        .iter()
        .filter(|event| event.block_number <= block)
        .cloned()
        .collect();
    let leaves: Vec<String> = events.iter().map(event_leaf).collect();
    Ok(Checkpoint {
        dataset_id: holderproof_types::DATASET_ID.into(),
        block_hash_start: fixture_block_hash(0),
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
    spec.hash()
}

/// Node-local replay result. Checkpoint metadata must agree along with balance.
pub(crate) fn fixture_fact(
    holder: Address,
    block: u64,
) -> LyquidResult<holderproof_types::BalanceFact> {
    Ok(holderproof_types::BalanceFact {
        query: make_query_spec(&fixture_token()?, holder, block),
        query_type: holderproof_types::QueryType::Balance,
        balance: fixture_balance(holder, block)?.to_string(),
        checkpoint: fixture_checkpoint(block)?,
    })
}
