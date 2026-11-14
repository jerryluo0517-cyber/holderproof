//! Governance policy is independent of airdrop policy, but consumes the same
//! immutable Indexer certificate. Votes are recorded once per proposal/holder.

use holderproof_gate_core::{certified_balance, configuration, eligibility::evaluate, invalid};
use lyquid::prelude::*;

state! {
    instance result_history: Vec<String> = Vec::new();
    network vote_count: u64 = 0;
    network initializer: Address = Address::ZERO;
    network indexer: Address = Address::ZERO;
    network holder: Address = Address::ZERO;
    network block: u64 = 0;
    network threshold: U256 = U256::ZERO;
    network proposal_id: u64 = 0;
    network voted: HashMap<String, bool> = new_hashmap();
    network eligible: bool = false;
    network balance: U256 = U256::ZERO;
    network query_hash: String = String::new();
    network fact_hash: String = String::new();
    network check_count: u64 = 0;
}

#[method::network(export = eth)]
fn constructor(ctx: &mut _) {
    *ctx.network.initializer = ctx.caller;
}

#[method::network(export = eth)]
fn configure(
    ctx: &mut _,
    indexer: Address,
    holder: Address,
    block: u64,
    threshold: U256,
    proposal_id: u64,
) -> LyquidResult<bool> {
    configuration::validate(ctx.caller, *ctx.network.initializer, indexer, holder)?;
    *ctx.network.indexer = indexer;
    *ctx.network.holder = holder;
    *ctx.network.block = block;
    *ctx.network.threshold = threshold;
    *ctx.network.proposal_id = proposal_id;
    *ctx.network.eligible = false;
    *ctx.network.balance = U256::ZERO;
    *ctx.network.query_hash = String::new();
    *ctx.network.fact_hash = String::new();
    *ctx.network.check_count = 0;
    Ok(true)
}

#[method::network(export = eth)]
fn check_eligible(ctx: &mut _) -> LyquidResult<bool> {
    let certificate = certified_balance(
        *ctx.network.indexer,
        *ctx.network.holder,
        *ctx.network.block,
    )?;
    let (balance, eligible) = evaluate(&certificate, *ctx.network.threshold)?;
    *ctx.network.balance = balance;
    *ctx.network.eligible = eligible;
    *ctx.network.query_hash = certificate.query_hash;
    *ctx.network.fact_hash = certificate.fact_hash;
    *ctx.network.check_count += 1;
    trigger!(record_result(), TriggerMode::Commit);
    Ok(eligible)
}

#[method::network(export = eth)]
fn vote(ctx: &mut _) -> LyquidResult<bool> {
    if ctx.caller != *ctx.network.holder {
        return Err(invalid("voter must match configured holder"));
    }
    let key = format!("{}|{}", *ctx.network.proposal_id, ctx.caller);
    if ctx.network.voted.contains_key(&key) {
        return Err(invalid("holder already voted on proposal"));
    }
    let certificate = certified_balance(*ctx.network.indexer, ctx.caller, *ctx.network.block)?;
    if !evaluate(&certificate, *ctx.network.threshold)?.1 {
        return Err(invalid("holder does not meet governance threshold"));
    }
    ctx.network.voted.insert(key, true);
    *ctx.network.vote_count += 1;
    Ok(true)
}

#[method::instance(export = http, method = "GET", path_prefix = "/api/result")]
fn http_result(ctx: &_, _req: http::Request) -> LyquidResult<http::Response> {
    let body = serde_json::to_vec(&serde_json::json!({
        "voteCount": *ctx.network.vote_count,
        "role": "GovernanceGateLyquid", "proposalId": *ctx.network.proposal_id,
        "holder": ctx.network.holder.to_string(), "block": *ctx.network.block,
        "threshold": ctx.network.threshold.to_string(), "balance": ctx.network.balance.to_string(),
        "eligible": *ctx.network.eligible, "queryHash": ctx.network.query_hash.to_string(),
        "factHash": ctx.network.fact_hash.to_string(), "checkCount": *ctx.network.check_count,
    }))
    .map_err(|err| invalid(err.to_string()))?;
    Ok(http::Response {
        status: 200,
        headers: vec![http::Header {
            name: "content-type".into(),
            value: b"application/json".to_vec(),
        }],
        body,
    })
}

#[method::instance]
fn record_result(ctx: &mut _) -> LyquidResult<()> {
    let item = serde_json::json!({
        "holder": ctx.network.holder.to_string(), "block": *ctx.network.block,
        "threshold": ctx.network.threshold.to_string(), "balance": ctx.network.balance.to_string(),
        "eligible": *ctx.network.eligible, "queryHash": ctx.network.query_hash.to_string(),
    })
    .to_string();
    let mut history = ctx.instance.result_history.write();
    if history.len() == 32 {
        history.remove(0);
    }
    history.push(item);
    Ok(())
}

#[method::instance(export = http, method = "GET", path_prefix = "/api/history")]
fn http_history(ctx: &_, _req: http::Request) -> LyquidResult<http::Response> {
    let history = ctx.instance.result_history.read();
    let body = serde_json::to_vec(&*history).map_err(|err| invalid(err.to_string()))?;
    Ok(http::Response {
        status: 200,
        headers: vec![],
        body,
    })
}
