use crate::invalid;
use crate::merkle::{hash_hex, merkle_root};
use holderproof_types::{
    CHAIN_ID, COMMITTEE_THRESHOLD, CREDIT_DATASET_ID, CREDIT_PROTOCOL, CertificateStatus,
    Checkpoint, CreditCertificate, CreditEvent, CreditFact, CreditMetrics, CreditParams,
    CreditQuerySpec, FIXTURE_END_BLOCK, NodeSignature, PARSER_VERSION, QueryType,
    fixture_block_hash,
};
use lyquid::prelude::*;
use lyquor_primitives::oracle::OracleCert;
use std::sync::LazyLock;

const CREDIT_FIXTURE_JSON: &str = include_str!("../../fixtures/credit_events.json");

static CREDIT_EVENTS: LazyLock<Result<Vec<CreditEvent>, String>> =
    LazyLock::new(|| parse_credit_events(CREDIT_FIXTURE_JSON));

pub(crate) fn parse_credit_events(json: &str) -> Result<Vec<CreditEvent>, String> {
    let mut events: Vec<CreditEvent> =
        serde_json::from_str(json).map_err(|err| format!("credit fixture JSON error: {err}"))?;
    events.sort_by_key(|e| (e.block_number, e.log_index));
    for (index, event) in events.iter().enumerate() {
        if event.chain_id != CHAIN_ID
            || event.protocol != CREDIT_PROTOCOL
            || event.parser_version != PARSER_VERSION
            || event.block_hash != fixture_block_hash(event.block_number)
            || event.block_number > FIXTURE_END_BLOCK
        {
            return Err(format!("invalid credit event metadata at {index}"));
        }
        if index > 0
            && events[index - 1].block_number == event.block_number
            && events[index - 1].log_index == event.log_index
        {
            return Err(format!("duplicate credit event position at {index}"));
        }
        if event.user.parse::<Address>().is_err()
            || event.counterparty.parse::<Address>().is_err()
            || event.amount.parse::<u128>().is_err()
        {
            return Err(format!("invalid credit address or amount at {index}"));
        }
    }
    Ok(events)
}

pub(crate) fn credit_events() -> LyquidResult<&'static [CreditEvent]> {
    match &*CREDIT_EVENTS {
        Ok(events) => Ok(events.as_slice()),
        Err(err) => Err(invalid(err.as_str())),
    }
}

pub(crate) fn credit_event_leaf(event: &CreditEvent) -> String {
    hash_hex(
        format!(
            "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
            event.chain_id,
            event.protocol,
            event.block_number,
            event.block_hash,
            event.tx_hash,
            event.log_index,
            event.user,
            event.counterparty,
            event.event_type,
            event.amount,
            event.parser_version
        )
        .as_bytes(),
    )
}

pub(crate) fn evaluate_credit(
    events: &[CreditEvent],
    user: Address,
    start_block: u64,
    end_block: u64,
    params: CreditParams,
) -> LyquidResult<CreditMetrics> {
    if end_block < start_block {
        return Err(invalid("credit end_block must be >= start_block"));
    }
    let target = user.to_string().to_ascii_lowercase();
    let mut unmatched: Vec<(u64, String, String, u128)> = Vec::new();
    let mut raw_volume: u128 = 0;
    let mut repay_count: u64 = 0;
    let mut liquidation_count: u64 = 0;

    for event in events {
        let b = event.block_number;
        if b < start_block || b > end_block || event.user.to_ascii_lowercase() != target {
            continue;
        }
        let amt = event
            .amount
            .parse::<u128>()
            .map_err(|_| invalid("invalid credit event amount"))?;
        match event.event_type.as_str() {
            "SwapBuy" | "SwapSell" => {
                raw_volume = raw_volume
                    .checked_add(amt)
                    .ok_or_else(|| invalid("raw volume overflow"))?;
                let cp = event.counterparty.to_ascii_lowercase();
                let mut rem = amt;
                for prev in &mut unmatched {
                    if rem > 0
                        && prev.3 > 0
                        && prev.1 == cp
                        && prev.2 != event.event_type
                        && b.saturating_sub(prev.0) <= params.wash_window
                    {
                        let matched = rem.min(prev.3);
                        rem -= matched;
                        prev.3 -= matched;
                    }
                }
                if rem > 0 {
                    unmatched.push((b, cp, event.event_type.clone(), rem));
                }
            }
            "Repay" => repay_count += 1,
            "Liquidation" => liquidation_count += 1,
            _ => {}
        }
    }

    let clean_volume: u128 = unmatched.iter().map(|item| item.3).sum();
    let wash_removed = raw_volume.saturating_sub(clean_volume);
    let log_term = (1u128 + clean_volume).ilog2() as u128;
    let numerator = (params.w1 as u128)
        .checked_mul(log_term)
        .and_then(|v| v.checked_add((params.w2 as u128) * (repay_count as u128)))
        .ok_or_else(|| invalid("credit numerator overflow"))?;
    let denominator = 1u128
        .checked_add((params.alpha as u128) * (liquidation_count as u128))
        .ok_or_else(|| invalid("credit denominator overflow"))?;
    let score = (numerator / denominator).min(1000) as u64;

    Ok(CreditMetrics {
        raw_volume: raw_volume.to_string(),
        clean_volume: clean_volume.to_string(),
        wash_volume_removed: wash_removed.to_string(),
        repay_count,
        liquidation_count,
        score,
    })
}

pub(crate) fn credit_checkpoint(start_block: u64, end_block: u64) -> LyquidResult<Checkpoint> {
    let events: Vec<&CreditEvent> = credit_events()?
        .iter()
        .filter(|e| e.block_number >= start_block && e.block_number <= end_block)
        .collect();
    let leaves: Vec<String> = events.iter().map(|e| credit_event_leaf(e)).collect();
    Ok(Checkpoint {
        dataset_id: CREDIT_DATASET_ID.into(),
        block_hash_start: fixture_block_hash(start_block),
        from_block: start_block,
        to_block: end_block,
        block_hash: fixture_block_hash(end_block),
        event_count: events.len() as u64,
        event_merkle_root: merkle_root(&leaves)?,
        parser_version: PARSER_VERSION.to_string(),
    })
}

pub(crate) fn make_credit_query(
    user: Address,
    start_block: u64,
    end_block: u64,
    params: CreditParams,
) -> CreditQuerySpec {
    CreditQuerySpec {
        chain_id: CHAIN_ID,
        protocol: CREDIT_PROTOCOL.to_string(),
        user: user.to_string().to_ascii_lowercase(),
        start_block,
        end_block,
        params,
        parser_version: PARSER_VERSION.to_string(),
    }
}

pub(crate) fn credit_fact(
    user: Address,
    start_block: u64,
    end_block: u64,
    params: CreditParams,
) -> LyquidResult<CreditFact> {
    let query = make_credit_query(user, start_block, end_block, params);
    let metrics = evaluate_credit(credit_events()?, user, start_block, end_block, params)?;
    let checkpoint = credit_checkpoint(start_block, end_block)?;
    Ok(CreditFact {
        query,
        query_type: QueryType::CreditScore,
        metrics,
        checkpoint,
    })
}

pub(crate) fn build_credit_certificate(
    fact: &CreditFact,
    node_signatures: Vec<NodeSignature>,
    runtime_certificate: &OracleCert,
    indexer: LyquidID,
    created_at: u64,
) -> LyquidResult<CreditCertificate> {
    let signer_ids: Vec<String> = node_signatures.iter().map(|s| s.node_id.clone()).collect();
    let has_dup = signer_ids
        .iter()
        .enumerate()
        .any(|(i, s)| signer_ids[..i].contains(s));
    if signer_ids.len() < COMMITTEE_THRESHOLD || has_dup {
        return Err(invalid("credit certificate requires distinct quorum signers"));
    }
    let query_hash = fact.query.hash();
    let runtime_certificate = serde_json::to_string(runtime_certificate)
        .map_err(|err| invalid(format!("runtime certificate JSON error: {err}")))?;
    Ok(CreditCertificate {
        query_type: fact.query_type,
        fact_hash: fact.hash(),
        node_signatures,
        runtime_certificate,
        network_state_id: format!("{indexer}@{created_at}"),
        certificate_id: query_hash.clone(),
        query_hash,
        query: fact.query.clone(),
        metrics: fact.metrics.clone(),
        checkpoint: fact.checkpoint.clone(),
        signer_ids,
        status: CertificateStatus::Finalized,
        created_at,
    })
}