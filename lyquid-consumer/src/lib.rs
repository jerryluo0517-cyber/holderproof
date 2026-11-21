//! HolderProof Consumer Lyquid.
//!
//! Reading order:
//! 1. `state!` holds the configured query and latest validated result.
//! 2. `configure` binds the Indexer, holder, block and threshold.
//! 3. `check_eligible` fetches the certificate from the Indexer.
//! 4. `holderproof-gate-core/certificate.rs` validates the trust rules.
//! 5. `holderproof-gate-core/eligibility.rs` parses the balance and computes eligibility.
//! 6. `http_endpoints.rs` exposes the diagnostic result.

use holderproof_gate_core::{certified_balance, configuration, eligibility::evaluate, invalid};
use lyquid::prelude::*;
mod http_endpoints;

state! {
    instance result_history: Vec<String> = Vec::new();
    network claim_count: u64 = 0;
    network claimed: HashMap<Address, bool> = new_hashmap();
    network initializer: Address = Address::ZERO;
    network indexer: Address = Address::ZERO;
    network expected_holder: Address = Address::ZERO;
    network expected_block: u64 = 0;
    network threshold: U256 = U256::ZERO;
    network last_balance: U256 = U256::ZERO;
    network last_eligible: bool = false;
    network check_count: u64 = 0;
    network last_query_hash: String = String::new();
    network last_block_hash: String = String::new();
    network last_event_merkle_root: String = String::new();
    network last_certificate_status: String = String::new();
    network last_signer_count: u64 = 0;
}

#[method::network(export = eth)]
fn constructor(ctx: &mut _) {
    *ctx.network.initializer = ctx.caller;
}

#[method::network(export = eth)]
fn configure(
    ctx: &mut _,
    indexer: Address,
    expected_holder: Address,
    expected_block: u64,
    threshold: U256,
) -> LyquidResult<bool> {
    configuration::validate(
        ctx.caller,
        *ctx.network.initializer,
        indexer,
        expected_holder,
    )?;
    // A result belongs to its configuration. Clear it before accepting a new
    // query so diagnostics cannot present the previous query as current.
    *ctx.network.last_balance = U256::ZERO;
    *ctx.network.last_eligible = false;
    *ctx.network.check_count = 0;
    *ctx.network.last_query_hash = String::new();
    *ctx.network.last_block_hash = String::new();
    *ctx.network.last_event_merkle_root = String::new();
    *ctx.network.last_certificate_status = String::new();
    *ctx.network.last_signer_count = 0;
    *ctx.network.indexer = indexer;
    *ctx.network.expected_holder = expected_holder;
    *ctx.network.expected_block = expected_block;
    *ctx.network.threshold = threshold;
    Ok(true)
}

/// Multi-user mode: configure policy without locking a single holder.
#[method::network(export = eth)]
fn configure_policy(
    ctx: &mut _,
    indexer: Address,
    expected_block: u64,
    threshold: U256,
) -> LyquidResult<bool> {
    if ctx.caller != *ctx.network.initializer {
        return Err(invalid("only initializer can configure consumer"));
    }
    if indexer == Address::ZERO {
        return Err(invalid("indexer must be nonzero address"));
    }
    *ctx.network.last_balance = U256::ZERO;
    *ctx.network.last_eligible = false;
    *ctx.network.check_count = 0;
    *ctx.network.last_query_hash = String::new();
    *ctx.network.last_block_hash = String::new();
    *ctx.network.last_event_merkle_root = String::new();
    *ctx.network.last_certificate_status = String::new();
    *ctx.network.last_signer_count = 0;
    *ctx.network.indexer = indexer;
    *ctx.network.expected_holder = Address::ZERO; // 0 表示开放多用户模式
    *ctx.network.expected_block = expected_block;
    *ctx.network.threshold = threshold;
    Ok(true)
}

#[method::network(export = eth)]
fn check_eligible_for(ctx: &mut _, holder: Address) -> LyquidResult<bool> {
    let indexer_address = *ctx.network.indexer;
    let block = *ctx.network.expected_block;
    let threshold = *ctx.network.threshold;
    if indexer_address == Address::ZERO {
        return Err(invalid("HolderProof consumer indexer is not configured"));
    }
    if holder == Address::ZERO {
        return Err(invalid("holder must be nonzero address"));
    }

    let certificate = certified_balance(indexer_address, holder, block)?;
    let (balance, eligible) = evaluate(&certificate, threshold)?;

    *ctx.network.expected_holder = if *ctx.network.expected_holder == Address::ZERO {
        Address::ZERO
    } else {
        holder
    };
    *ctx.network.last_balance = balance;
    *ctx.network.last_eligible = eligible;
    *ctx.network.check_count += 1;
    *ctx.network.last_query_hash = certificate.query_hash;
    *ctx.network.last_block_hash = certificate.checkpoint.block_hash;
    *ctx.network.last_event_merkle_root = certificate.checkpoint.event_merkle_root;
    *ctx.network.last_certificate_status = certificate.status.to_string();
    *ctx.network.last_signer_count = certificate.signer_ids.len() as u64;
    trigger!(record_result(), TriggerMode::Commit);
    Ok(eligible)
}

#[method::network(export = eth)]
fn check_eligible(ctx: &mut _) -> LyquidResult<bool> {
    let indexer_address = *ctx.network.indexer;
    let holder = *ctx.network.expected_holder;
    let block = *ctx.network.expected_block;
    let threshold = *ctx.network.threshold;
    if indexer_address == Address::ZERO {
        return Err(invalid("HolderProof consumer indexer is not configured"));
    }

    let certificate = certified_balance(indexer_address, holder, block)?;
    let (balance, eligible) = evaluate(&certificate, threshold)?;

    *ctx.network.last_balance = balance;
    *ctx.network.last_eligible = eligible;
    *ctx.network.check_count += 1;
    *ctx.network.last_query_hash = certificate.query_hash;
    *ctx.network.last_block_hash = certificate.checkpoint.block_hash;
    *ctx.network.last_event_merkle_root = certificate.checkpoint.event_merkle_root;
    *ctx.network.last_certificate_status = certificate.status.to_string();
    *ctx.network.last_signer_count = certificate.signer_ids.len() as u64;
    trigger!(record_result(), TriggerMode::Commit);
    Ok(eligible)
}

// Records an airdrop entitlement, not an ERC-20 payout. The holder must be
// the sequenced caller; an operator cannot claim using someone else's proof.
#[method::network(export = eth)]
fn claim(ctx: &mut _) -> LyquidResult<bool> {
    let locked = *ctx.network.expected_holder;
    if locked != Address::ZERO && ctx.caller != locked {
        return Err(invalid("claim caller must match configured holder"));
    }
    if ctx
        .network
        .claimed
        .get(&ctx.caller)
        .copied()
        .unwrap_or(false)
    {
        return Err(invalid("airdrop entitlement already claimed"));
    }
    let certificate = certified_balance(
        *ctx.network.indexer,
        ctx.caller,
        *ctx.network.expected_block,
    )?;
    if !evaluate(&certificate, *ctx.network.threshold)?.1 {
        return Err(invalid("holder does not meet airdrop threshold"));
    }
    ctx.network.claimed.insert(ctx.caller, true);
    *ctx.network.claim_count += 1;
    Ok(true)
}

// Diagnostic history is per instance; it is not part of canonical gate policy.
#[method::instance]
fn record_result(ctx: &mut _) -> LyquidResult<()> {
    let item = serde_json::json!({
        "holder": ctx.network.expected_holder.to_string(),
        "block": *ctx.network.expected_block,
        "threshold": ctx.network.threshold.to_string(),
        "balance": ctx.network.last_balance.to_string(),
        "eligible": *ctx.network.last_eligible,
        "queryHash": ctx.network.last_query_hash.to_string(),
    })
    .to_string();
    let mut history = ctx.instance.result_history.write();
    if history.len() == 32 {
        history.remove(0);
    }
    history.push(item);
    Ok(())
}
