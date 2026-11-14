//! HolderProof Indexer Lyquid.
//!
//! Reading order:
//! 1. `state!` below shows shared network state.
//! 2. `set_query` selects the holder and historical block.
//! 3. `propose` computes the fixture balance on every committee node.
//! 4. `aggregate` requires two matching complete-fact proposals.
//! 5. `update_certified_balance` stores the full certificate.
//! 6. `get_certificate_json` exposes the certificate to the Consumer.
//! 7. `report_balance` starts the certified-call workflow.
//!
//! Pure data and algorithms live outside this file:
//! - `fixture.rs`: fixture parsing, balance and TWAB
//! - `merkle.rs`: event leaves and Merkle root
//! - `models.rs`: committee proposal
//! - `holderproof-types`: shared certificate wire types

use fixture::{fixture_fact, fixture_token, fixture_twab, make_query_spec, query_hash};
use holderproof_types::{COMMITTEE_THRESHOLD, CertificateStatus, NodeSignature, QueryCertificate};
use lyquid::prelude::*;
use models::{BalanceProposal, agreeing_proposal, valid_committee};

/// Build a runtime error without the `LyquidError::LyquorRuntime(...into())`
/// boilerplate.
pub(crate) fn invalid(reason: impl Into<String>) -> LyquidError {
    LyquidError::LyquorRuntime(reason.into())
}

mod certificate_builder;
mod fixture;
mod http_endpoints;
mod merkle;
mod models;
#[cfg(test)]
mod tests;

state! {
    // Per-node replay cache is never sequenced as shared state.
    instance proposal_cache: HashMap<String, holderproof_types::BalanceFact> = new_hashmap();
    network initializer: Address = Address::ZERO;
    network oracle balance_proof;

    network query_holder: Address = Address::ZERO;
    network query_block: u64 = 0;

    // Plain fallback result retained for the cross-Lyquid smoke test.
    network snapshot_holder: Address = Address::ZERO;
    network snapshot_block: u64 = 0;
    network snapshot_balance: U256 = U256::ZERO;

    // Committee-certified result.
    network certified_holder: Address = Address::ZERO;
    network certified_block: u64 = 0;
    network certified_balance: U256 = U256::ZERO;
    network certified_signer_count: u64 = 0;

    // Full certificate persistence keyed by query hash.
    network latest_query_hash: String = String::new();
    network certificates: HashMap<String, QueryCertificate> = new_hashmap();
}

#[method::network(export = eth)]
fn constructor(ctx: &mut _) {
    *ctx.network.initializer = ctx.caller;
}

#[method::network(export = eth)]
fn set_query(ctx: &mut _, holder: Address, block: u64) -> LyquidResult<bool> {
    if holder == Address::ZERO {
        return Err(invalid("holder cannot be the zero address"));
    }
    if block > holderproof_types::FIXTURE_END_BLOCK {
        return Err(invalid("block exceeds finalized fixture range"));
    }
    *ctx.network.query_holder = holder;
    *ctx.network.query_block = block;
    Ok(true)
}

// Publish a balance fact for the first cross-Lyquid integration check.
// This is intentionally a plain network write. The committee path is the
// authoritative path used by the Consumer.
#[method::network(export = eth)]
fn publish_balance(ctx: &mut _, holder: Address, block: u64, balance: U256) -> LyquidResult<bool> {
    *ctx.network.snapshot_holder = holder;
    *ctx.network.snapshot_block = block;
    *ctx.network.snapshot_balance = balance;
    lyquid::println!(
        "holderproof-indexer: published balance {:?} for {:?} at block {}",
        balance,
        holder,
        block
    );
    Ok(true)
}

// Legacy smoke-test balance. Production gate paths read certificates instead.
#[method::network(export = eth)]
fn get_balance(ctx: &_, holder: Address, block: u64) -> LyquidResult<U256> {
    if holder == *ctx.network.snapshot_holder && block == *ctx.network.snapshot_block {
        Ok(*ctx.network.snapshot_balance)
    } else {
        Err(invalid("HolderProof snapshot not found"))
    }
}

// Consumer-facing read of the committee-certified result.
#[method::network(export = eth)]
fn get_certified_balance(ctx: &_, holder: Address, block: u64) -> LyquidResult<U256> {
    let key = make_query_spec(&fixture_token()?, holder, block).hash();
    let certificate = ctx
        .network
        .certificates
        .get(&key)
        .ok_or_else(|| invalid("certificate not found"))?;
    if certificate.status != CertificateStatus::Finalized
        || certificate.signer_ids.len() < COMMITTEE_THRESHOLD
    {
        return Err(invalid("certificate is not currently finalized"));
    }
    certificate
        .balance
        .parse::<U256>()
        .map_err(|_| invalid("invalid certified balance"))
}

// Configure exactly three committee members with threshold two.
#[method::network(export = eth)]
fn configure_committee(ctx: &mut _, node_ids: Vec<NodeID>) -> LyquidResult<bool> {
    if ctx.caller != *ctx.network.initializer {
        return Err(invalid("only initializer can configure committee"));
    }
    if !valid_committee(&node_ids) {
        return Err(invalid(
            "committee must contain exactly three distinct nodes",
        ));
    }

    let target = OracleTarget {
        seq_id: lyquor_api::sequence_backend_id()?,
        target: OracleServiceTarget::LVM(ctx.lyquid_id),
    };
    // n / 2 + 1: equals COMMITTEE_THRESHOLD (2) for three members.
    let threshold = u16::try_from(node_ids.len() / 2 + 1)
        .map_err(|_| invalid("committee threshold overflow"))?;

    if !ctx
        .network
        .balance_proof
        .clone()
        .initialize(&mut ctx, target, node_ids, threshold)
    {
        return Err(invalid("balance committee initialization failed"));
    }
    Ok(true)
}

// Each committee node independently replays the embedded Transfer fixture.
#[method::instance(group = oracle::two_phase::balance_proof)]
fn propose(
    ctx: &mut _,
    _avg_num: u16,
    _target: OracleTarget,
    holder: Address,
    block: u64,
) -> LyquidResult<BalanceProposal> {
    // Parameters are captured in the signed proposal round, so changing the
    // selected query while this round runs cannot change its meaning.
    if holder == Address::ZERO {
        return Err(invalid("query is not configured"));
    }
    let proposal = fixture_fact(holder, block)?;
    ctx.instance
        .proposal_cache
        .write()
        .insert(proposal.query.hash(), proposal.clone());
    lyquid::println!("holderproof-indexer: fixture proposal = {:?}", proposal);
    Ok(proposal)
}

#[method::instance(group = oracle::two_phase::balance_proof)]
fn aggregate(ctx: &_) -> LyquidResult<Option<CertifiedCallParams>> {
    let init = decode_by_fields!(ctx.init, avg_num: u16, target: OracleTarget, holder: Address, block: u64)
        .ok_or_else(|| invalid("invalid balance committee init"))?;
    if init.target.seq_id != lyquor_api::sequence_backend_id()?
        || init.target.target != OracleServiceTarget::LVM(ctx.lyquid_id)
    {
        return Ok(None);
    }

    let mut proposals = Vec::new();
    for input in ctx.inputs {
        if let Some(proposal) = lyquor_primitives::decode_object::<BalanceProposal>(&input.input) {
            proposals.push((input.from, proposal));
        }
    }
    // The proposal threshold must agree with the fixed Step 4 committee policy.
    if init.avg_num as usize != COMMITTEE_THRESHOLD {
        return Err(invalid("unexpected proposal threshold"));
    }
    let Some(agreed_fact) = agreeing_proposal(&proposals, COMMITTEE_THRESHOLD) else {
        return Ok(None);
    };

    if agreed_fact.query.holder != init.holder.to_string().to_ascii_lowercase()
        || agreed_fact.query.block != init.block
    {
        return Err(invalid("committee fact does not match the requested query"));
    }
    // Commit the complete voted fact, not only its balance. Native signatures
    // now cover the checkpoint and query fields in this exact JSON payload.
    let fact_json = serde_json::to_string(agreed_fact).map_err(|err| invalid(err.to_string()))?;
    let input = encode_by_fields!(fact_json: String = fact_json).into();
    Ok(Some(CertifiedCallParams {
        origin: Address::ZERO,
        method: "update_certified_balance".to_string(),
        input,
        target: init.target,
    }))
}

#[method::network(group = oracle::certified::balance_proof::two_phase)]
fn update_certified_balance(ctx: &mut _, fact_json: String) -> LyquidResult<bool> {
    let fact: holderproof_types::BalanceFact = serde_json::from_str(&fact_json)
        .map_err(|err| invalid(format!("invalid signed fact: {err}")))?;
    let holder = fact
        .query
        .holder
        .parse::<Address>()
        .map_err(|_| invalid("invalid fact holder"))?;
    let block = fact.query.block;
    // Defense in depth: the committed fact must match deterministic replay.
    if fact != fixture_fact(holder, block)? {
        return Err(invalid("signed fact does not match fixture replay"));
    }
    if ctx.cert.signers.len() != ctx.cert.signatures.len() {
        return Err(invalid("runtime signer/signature count mismatch"));
    }
    let mut node_signatures = Vec::new();
    let runtime_cert = ctx.cert.clone();
    for (signer_id, signature) in runtime_cert.signers.iter().zip(&runtime_cert.signatures) {
        let node_id = ctx
            .signer_node_id(*signer_id as u64)
            .ok_or_else(|| invalid("runtime signer is not in the active committee"))?;
        node_signatures.push(NodeSignature {
            node_id: node_id.to_string(),
            signer_id: *signer_id,
            signature: hex::encode(signature),
        });
    }
    let balance = fact
        .balance
        .parse::<U256>()
        .map_err(|_| invalid("invalid fact balance"))?;
    let certificate_hash = fact.query.hash();
    // Repeated identical queries return the original certificate, including its
    // creation position and signatures, rather than silently replacing it.
    if let Some(existing) = ctx.network.certificates.get(&certificate_hash) {
        if existing.fact_hash != fact.hash() {
            return Err(invalid("conflicting certificate for query"));
        }
        *ctx.network.latest_query_hash = certificate_hash;
        return Ok(true);
    }
    let created_at = lyquor_api::chain_pos()?.block();
    let certificate = certificate_builder::build(
        &fact,
        node_signatures,
        &runtime_cert,
        ctx.lyquid_id,
        created_at,
    )?;
    let signer_count = certificate.signer_ids.len() as u64;

    *ctx.network.certified_holder = holder;
    *ctx.network.certified_block = block;
    *ctx.network.certified_balance = balance;
    *ctx.network.certified_signer_count = signer_count;
    *ctx.network.latest_query_hash = certificate_hash.clone();
    ctx.network
        .certificates
        .insert(certificate_hash, certificate.clone());

    lyquid::println!(
        "holderproof-indexer: finalized certificate {:?} with {} signers",
        certificate,
        signer_count
    );
    Ok(true)
}

#[method::network(export = eth)]
fn get_certificate_json(ctx: &_, holder: Address, block: u64) -> LyquidResult<String> {
    let token = fixture_token()?;
    let query = make_query_spec(&token, holder, block);
    let certificate_hash = query_hash(&query);
    let certificate = ctx
        .network
        .certificates
        .get(&certificate_hash)
        .ok_or_else(|| invalid("certificate not found"))?;
    serde_json::to_string(certificate)
        .map_err(|err| invalid(format!("certificate JSON error: {err}")))
}

#[method::network(export = eth)]
fn get_twab(_ctx: &_, holder: Address, start_block: u64, end_block: u64) -> LyquidResult<U256> {
    fixture_twab(holder, start_block, end_block, 1_000_000)
}

#[method::instance(export = eth)]
fn report_balance(ctx: &mut _) -> LyquidResult<bool> {
    let target = OracleTarget {
        seq_id: lyquor_api::sequence_backend_id()?,
        target: OracleServiceTarget::LVM(ctx.lyquid_id),
    };
    let oracle = ctx.network.balance_proof.clone();
    let holder = *ctx.network.query_holder;
    let block = *ctx.network.query_block;
    let call = oracle.propose_and_certify(
        &mut ctx,
        target,
        encode_by_fields!(avg_num: u16 = COMMITTEE_THRESHOLD as u16, target: OracleTarget = target,
            holder: Address = holder, block: u64 = block)
        .into(),
        None,
        None,
    )?;
    if let Some(call) = call {
        submit_certified_call!(call)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

// Explicit P0 query interface; set_query remains a backward-compatible helper.
#[method::network(export = eth)]
fn proof(ctx: &mut _, token: Address, holder: Address, block: u64) -> LyquidResult<bool> {
    if token.to_string().to_ascii_lowercase() != holderproof_types::FIXTURE_TOKEN {
        return Err(invalid("unsupported token"));
    }
    if holder == Address::ZERO || block > holderproof_types::FIXTURE_END_BLOCK {
        return Err(invalid("invalid holder or block outside finalized fixture"));
    }
    *ctx.network.query_holder = holder;
    *ctx.network.query_block = block;
    Ok(true)
}

#[method::network(export = eth)]
fn get_query_json(ctx: &_) -> LyquidResult<String> {
    serde_json::to_string(&make_query_spec(
        &fixture_token()?,
        *ctx.network.query_holder,
        *ctx.network.query_block,
    ))
    .map_err(|err| invalid(err.to_string()))
}

// Revocation changes current policy status without replacing the signed fact.
#[method::network(export = eth)]
fn invalidate_certificate(ctx: &mut _, holder: Address, block: u64) -> LyquidResult<bool> {
    if ctx.caller != *ctx.network.initializer {
        return Err(invalid("only initializer can invalidate certificate"));
    }
    let key = make_query_spec(&fixture_token()?, holder, block).hash();
    let certificate = ctx
        .network
        .certificates
        .get_mut(&key)
        .ok_or_else(|| invalid("certificate not found"))?;
    certificate.status = CertificateStatus::Invalidated;
    Ok(true)
}
