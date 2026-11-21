//! VeriCredit consumer Lyquid: synchronously reads certified credit scores via
//! atomic `call!` to quote dynamic DEX swap fees (bps) and lending LTV (bps).

use holderproof_gate_core::{
    credit::{certified_credit, evaluate_tier},
    invalid,
};
use lyquid::prelude::*;

state! {
    instance quote_history: Vec<String> = Vec::new();
    network initializer: Address = Address::ZERO;
    network indexer: Address = Address::ZERO;
    network start_block: u64 = 0;
    network end_block: u64 = holderproof_types::FIXTURE_END_BLOCK;
    network last_user: Address = Address::ZERO;
    network last_score: u64 = 0;
    network last_fee_bps: u64 = 30;
    network last_ltv_bps: u64 = 6000;
    network last_tier: String = String::new();
    network last_clean_volume: String = String::new();
    network last_wash_removed: String = String::new();
    network last_query_hash: String = String::new();
    network quote_count: u64 = 0;
    network last_fallback: bool = false;
}

#[method::network(export = eth)]
fn constructor(ctx: &mut _) {
    *ctx.network.initializer = ctx.caller;
}

#[method::network(export = eth)]
fn configure(
    ctx: &mut _,
    indexer: Address,
    start_block: u64,
    end_block: u64,
) -> LyquidResult<bool> {
    if ctx.caller != *ctx.network.initializer {
        return Err(invalid("only initializer can configure credit gate"));
    }
    if indexer == Address::ZERO {
        return Err(invalid("indexer must be nonzero"));
    }
    if start_block > end_block || end_block > holderproof_types::FIXTURE_END_BLOCK {
        return Err(invalid("invalid credit gate window"));
    }
    *ctx.network.indexer = indexer;
    *ctx.network.start_block = start_block;
    *ctx.network.end_block = end_block;
    *ctx.network.quote_count = 0;
    Ok(true)
}

/// Same-call synchronous credit check: returns dynamic DEX fee in bps.
#[method::network(export = eth)]
fn quote_for(ctx: &mut _, user: Address) -> LyquidResult<u64> {
    let indexer = *ctx.network.indexer;
    if indexer == Address::ZERO {
        return Err(invalid("credit gate indexer is not configured"));
    }
    let cert = certified_credit(
        indexer,
        user,
        *ctx.network.start_block,
        *ctx.network.end_block,
    )?;
    let tier = evaluate_tier(cert.metrics.score);
    *ctx.network.last_user = user;
    *ctx.network.last_score = cert.metrics.score;
    *ctx.network.last_fee_bps = tier.fee_bps;
    *ctx.network.last_ltv_bps = tier.ltv_bps;
    *ctx.network.last_tier = tier.label.to_string();
    *ctx.network.last_clean_volume = cert.metrics.clean_volume;
    *ctx.network.last_wash_removed = cert.metrics.wash_volume_removed;
    *ctx.network.last_query_hash = cert.query_hash;
    *ctx.network.last_fallback = false;
    *ctx.network.quote_count += 1;
    trigger!(record_quote(), TriggerMode::Commit);
    Ok(tier.fee_bps)
}

/// Fail-safe quote: uses certified credit when valid, otherwise falls back
/// to conservative default risk parameters (Base: 30 bps fee, 60% LTV).
#[method::network(export = eth)]
fn quote_or_fallback(ctx: &mut _, user: Address) -> LyquidResult<u64> {
    let indexer = *ctx.network.indexer;
    if indexer == Address::ZERO {
        return Err(invalid("credit gate indexer is not configured"));
    }
    match certified_credit(
        indexer,
        user,
        *ctx.network.start_block,
        *ctx.network.end_block,
    ) {
        Ok(cert) => {
            let tier = evaluate_tier(cert.metrics.score);
            *ctx.network.last_user = user;
            *ctx.network.last_score = cert.metrics.score;
            *ctx.network.last_fee_bps = tier.fee_bps;
            *ctx.network.last_ltv_bps = tier.ltv_bps;
            *ctx.network.last_tier = tier.label.to_string();
            *ctx.network.last_clean_volume = cert.metrics.clean_volume;
            *ctx.network.last_wash_removed = cert.metrics.wash_volume_removed;
            *ctx.network.last_query_hash = cert.query_hash;
            *ctx.network.last_fallback = false;
            *ctx.network.quote_count += 1;
            trigger!(record_quote(), TriggerMode::Commit);
            Ok(tier.fee_bps)
        }
        Err(_) => {
            let default_tier = evaluate_tier(0); // Base: 30 bps, 6000 bps LTV
            *ctx.network.last_user = user;
            *ctx.network.last_score = 0;
            *ctx.network.last_fee_bps = default_tier.fee_bps;
            *ctx.network.last_ltv_bps = default_tier.ltv_bps;
            *ctx.network.last_tier = "Base (Fallback)".to_string();
            *ctx.network.last_clean_volume = "0".to_string();
            *ctx.network.last_wash_removed = "0".to_string();
            *ctx.network.last_query_hash = String::new();
            *ctx.network.last_fallback = true;
            *ctx.network.quote_count += 1;
            trigger!(record_quote(), TriggerMode::Commit);
            Ok(default_tier.fee_bps)
        }
    }
}

#[method::instance]
fn record_quote(ctx: &mut _) -> LyquidResult<()> {
    let item = serde_json::json!({
        "user": ctx.network.last_user.to_string(),
        "score": *ctx.network.last_score,
        "tier": ctx.network.last_tier.to_string(),
        "feeBps": *ctx.network.last_fee_bps,
        "ltvBps": *ctx.network.last_ltv_bps,
        "cleanVolume": ctx.network.last_clean_volume.to_string(),
        "washRemoved": ctx.network.last_wash_removed.to_string(),
        "queryHash": ctx.network.last_query_hash.to_string(),
        "fallback": *ctx.network.last_fallback,
    })
    .to_string();
    let mut history = ctx.instance.quote_history.write();
    if history.len() == 32 {
        history.remove(0);
    }
    history.push(item);
    Ok(())
}

fn json_response(body: Vec<u8>) -> http::Response {
    http::Response {
        status: 200,
        headers: vec![
            http::Header {
                name: "content-type".into(),
                value: b"application/json; charset=utf-8".to_vec(),
            },
            http::Header {
                name: "access-control-allow-origin".into(),
                value: b"*".to_vec(),
            },
        ],
        body,
    }
}

#[method::instance(export = http, method = "GET", path_prefix = "/api/credit")]
fn http_credit(ctx: &_, _req: http::Request) -> LyquidResult<http::Response> {
    let body = serde_json::to_vec(&serde_json::json!({
        "role": "VeriCreditGateLyquid",
        "indexer": ctx.network.indexer.to_string(),
        "window": [*ctx.network.start_block, *ctx.network.end_block],
        "user": ctx.network.last_user.to_string(),
        "score": *ctx.network.last_score,
        "tier": ctx.network.last_tier.to_string(),
        "feeBps": *ctx.network.last_fee_bps,
        "ltvBps": *ctx.network.last_ltv_bps,
        "cleanVolume": ctx.network.last_clean_volume.to_string(),
        "washRemoved": ctx.network.last_wash_removed.to_string(),
        "queryHash": ctx.network.last_query_hash.to_string(),
        "quoteCount": *ctx.network.quote_count,
        "fallback": *ctx.network.last_fallback,
    }))
    .map_err(|err| invalid(err.to_string()))?;
    Ok(json_response(body))
}

#[method::instance(export = http, method = "GET", path_prefix = "/api/history")]
fn http_history(ctx: &_, _req: http::Request) -> LyquidResult<http::Response> {
    let history = ctx.instance.quote_history.read();
    let body = serde_json::to_vec(&*history).map_err(|err| invalid(err.to_string()))?;
    Ok(json_response(body))
}