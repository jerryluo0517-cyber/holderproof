use crate::fixture::replay_balance;
use crate::invalid;
use crate::merkle::{event_leaf, merkle_root};
use holderproof_types::{
    BalanceFact, CHAIN_ID, Checkpoint, PARSER_VERSION, QuerySpec, QueryType, SEPOLIA_DATASET_ID,
    TransferEvent,
};
use lyquid::prelude::*;
use serde::Deserialize;
use std::sync::LazyLock;

const SEPOLIA_EVENTS_JSON: &str = include_str!("../../fixtures/sepolia_transfers.json");
const SEPOLIA_META_JSON: &str = include_str!("../../fixtures/sepolia_expected.json");

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct SepoliaMeta {
    pub token: String,
    pub from_block: u64,
    pub to_block: u64,
    pub block_hash_start: String,
    pub block_hash: String,
}

static SEPOLIA_META: LazyLock<Result<SepoliaMeta, String>> = LazyLock::new(|| {
    serde_json::from_str(SEPOLIA_META_JSON).map_err(|e| format!("sepolia meta error: {e}"))
});

static SEPOLIA_EVENTS: LazyLock<Result<Vec<TransferEvent>, String>> = LazyLock::new(|| {
    let mut events: Vec<TransferEvent> = serde_json::from_str(SEPOLIA_EVENTS_JSON)
        .map_err(|e| format!("sepolia events JSON error: {e}"))?;
    events.sort_by_key(|e| (e.block_number, e.log_index));
    for (i, ev) in events.iter().enumerate() {
        if ev.chain_id != CHAIN_ID || ev.parser_version != PARSER_VERSION {
            return Err(format!("invalid sepolia metadata at event {i}"));
        }
        if i > 0
            && events[i - 1].block_number == ev.block_number
            && events[i - 1].log_index == ev.log_index
        {
            return Err(format!("duplicate sepolia position at event {i}"));
        }
        if ev.token.parse::<Address>().is_err()
            || ev.from.parse::<Address>().is_err()
            || ev.to.parse::<Address>().is_err()
            || ev.amount.parse::<U256>().is_err()
        {
            return Err(format!("invalid sepolia address or amount at event {i}"));
        }
    }
    Ok(events)
});

pub(crate) fn sepolia_meta() -> LyquidResult<&'static SepoliaMeta> {
    match &*SEPOLIA_META {
        Ok(m) => Ok(m),
        Err(e) => Err(invalid(e.as_str())),
    }
}

pub(crate) fn sepolia_events() -> LyquidResult<&'static [TransferEvent]> {
    match &*SEPOLIA_EVENTS {
        Ok(ev) => Ok(ev.as_slice()),
        Err(e) => Err(invalid(e.as_str())),
    }
}

pub(crate) fn sepolia_query(holder: Address) -> LyquidResult<QuerySpec> {
    let meta = sepolia_meta()?;
    Ok(QuerySpec {
        chain_id: CHAIN_ID,
        token: meta.token.to_ascii_lowercase(),
        holder: holder.to_string().to_ascii_lowercase(),
        block: meta.to_block,
        parser_version: PARSER_VERSION.to_string(),
    })
}

pub(crate) fn sepolia_fact(holder: Address) -> LyquidResult<BalanceFact> {
    let meta = sepolia_meta()?;
    let events = sepolia_events()?;
    let balance = replay_balance(events, holder, meta.to_block)?;
    let leaves: Vec<String> = events.iter().map(event_leaf).collect();
    let checkpoint = Checkpoint {
        dataset_id: SEPOLIA_DATASET_ID.to_string(),
        block_hash_start: meta.block_hash_start.to_ascii_lowercase(),
        from_block: meta.from_block,
        to_block: meta.to_block,
        block_hash: meta.block_hash.to_ascii_lowercase(),
        event_count: events.len() as u64,
        event_merkle_root: merkle_root(&leaves)?,
        parser_version: PARSER_VERSION.to_string(),
    };
    Ok(BalanceFact {
        query: sepolia_query(holder)?,
        query_type: QueryType::Balance,
        balance: balance.to_string(),
        checkpoint,
    })
}