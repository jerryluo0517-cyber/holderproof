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
use holderproof_types::{CreditCertificate, CreditFact, CreditParams, EpochCheckpoint, EpochFact};

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
mod credit;
mod sepolia;

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

    // VeriCredit-Idx enhancement state (coexists with Balance P0).
    instance credit_proposal_cache: HashMap<String, CreditFact> = new_hashmap();
    network oracle credit_proof;
    network credit_w1: u64 = holderproof_types::DEFAULT_CREDIT_W1;
    network credit_w2: u64 = holderproof_types::DEFAULT_CREDIT_W2;
    network credit_alpha: u64 = holderproof_types::DEFAULT_CREDIT_ALPHA;
    network credit_wash_window: u64 = holderproof_types::DEFAULT_WASH_WINDOW;
    network credit_query_user: Address = Address::ZERO;
    network credit_query_start: u64 = 0;
    network credit_query_end: u64 = holderproof_types::FIXTURE_END_BLOCK;
    network latest_credit_query_hash: String = String::new();
    network credit_certificates: HashMap<String, CreditCertificate> = new_hashmap();

    // Layer 2: Periodic global Epoch Checkpoints committed to network state.
    network oracle checkpoint_proof;
    network epoch_interval: u64 = holderproof_types::DEFAULT_EPOCH_INTERVAL;
    network pending_epoch_block: u64 = 0;
    network latest_epoch_block: u64 = 0;
    network epoch_checkpoints: HashMap<u64, EpochCheckpoint> = new_hashmap();

    // Dual-track Sepolia RPC verified balance state.
    network oracle sepolia_proof;
    network sepolia_query_holder: Address = Address::ZERO;
    network latest_sepolia_query_hash: String = String::new();
    network sepolia_certificates: HashMap<String, QueryCertificate> = new_hashmap();
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
        .initialize(&mut ctx, target, node_ids.clone(), threshold)
    {
        return Err(invalid("balance committee initialization failed"));
    }
    if !ctx
        .network
        .credit_proof
        .clone()
        .initialize(&mut ctx, target, node_ids.clone(), threshold)
    {
        return Err(invalid("credit committee initialization failed"));
    }
    if !ctx
        .network
        .checkpoint_proof
        .clone()
        .initialize(&mut ctx, target, node_ids.clone(), threshold)
    {
        return Err(invalid("checkpoint committee initialization failed"));
    }
    if !ctx
        .network
        .sepolia_proof
        .clone()
        .initialize(&mut ctx, target, node_ids, threshold)
    {
        return Err(invalid("sepolia committee initialization failed"));
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
    /*let key = make_query_spec(&fixture_token()?, holder, block).hash();
    if let Some(existing) = ctx.network.certificates.get(&key) {
        if existing.status == CertificateStatus::Finalized {
            return Ok(true);
        }
    }*/
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

#[method::network(export = eth)]
fn configure_credit_params(
    ctx: &mut _,
    w1: u64,
    w2: u64,
    alpha: u64,
    wash_window: u64,
) -> LyquidResult<bool> {
    if ctx.caller != *ctx.network.initializer {
        return Err(invalid("only initializer can configure credit params"));
    }
    if w1 == 0 && w2 == 0 {
        return Err(invalid("w1 and w2 cannot both be zero"));
    }
    *ctx.network.credit_w1 = w1;
    *ctx.network.credit_w2 = w2;
    *ctx.network.credit_alpha = alpha;
    *ctx.network.credit_wash_window = wash_window;
    Ok(true)
}

#[method::network(export = eth)]
fn set_credit_query(
    ctx: &mut _,
    user: Address,
    start_block: u64,
    end_block: u64,
) -> LyquidResult<bool> {
    if user == Address::ZERO {
        return Err(invalid("credit user cannot be the zero address"));
    }
    if start_block > end_block || end_block > holderproof_types::FIXTURE_END_BLOCK {
        return Err(invalid("invalid credit query block window"));
    }
    *ctx.network.credit_query_user = user;
    *ctx.network.credit_query_start = start_block;
    *ctx.network.credit_query_end = end_block;
    Ok(true)
}

mod credit_oracle {
    use super::*;

    #[method::instance(group = oracle::two_phase::credit_proof)]
    fn propose(
        ctx: &mut _,
        _avg_num: u16,
        _target: OracleTarget,
        user: Address,
        start_block: u64,
        end_block: u64,
        w1: u64,
        w2: u64,
        alpha: u64,
        wash_window: u64,
    ) -> LyquidResult<CreditFact> {
        if user == Address::ZERO {
            return Err(invalid("credit query is not configured"));
        }
        let params = CreditParams { w1, w2, alpha, wash_window };
        let proposal = credit::credit_fact(user, start_block, end_block, params)?;
        ctx.instance
            .credit_proposal_cache
            .write()
            .insert(proposal.query.hash(), proposal.clone());
        Ok(proposal)
    }

    #[method::instance(group = oracle::two_phase::credit_proof)]
    fn aggregate(ctx: &_) -> LyquidResult<Option<CertifiedCallParams>> {
        let init = decode_by_fields!(
            ctx.init,
            avg_num: u16,
            target: OracleTarget,
            user: Address,
            start_block: u64,
            end_block: u64,
            w1: u64,
            w2: u64,
            alpha: u64,
            wash_window: u64
        )
        .ok_or_else(|| invalid("invalid credit committee init"))?;
        if init.target.seq_id != lyquor_api::sequence_backend_id()?
            || init.target.target != OracleServiceTarget::LVM(ctx.lyquid_id)
        {
            return Ok(None);
        }
        if init.avg_num as usize != COMMITTEE_THRESHOLD {
            return Err(invalid("unexpected credit proposal threshold"));
        }
        let mut proposals: Vec<(_, CreditFact)> = Vec::new();
        for input in ctx.inputs {
            if let Some(proposal) = lyquor_primitives::decode_object::<CreditFact>(&input.input) {
                proposals.push((input.from, proposal));
            }
        }
        let Some(agreed) = agreeing_proposal(&proposals, COMMITTEE_THRESHOLD) else {
            return Ok(None);
        };
        let expected_params = CreditParams {
            w1: init.w1,
            w2: init.w2,
            alpha: init.alpha,
            wash_window: init.wash_window,
        };
        if agreed.query.user != init.user.to_string().to_ascii_lowercase()
            || agreed.query.start_block != init.start_block
            || agreed.query.end_block != init.end_block
            || agreed.query.params != expected_params
        {
            return Err(invalid("credit fact does not match requested query"));
        }
        let fact_json = serde_json::to_string(agreed).map_err(|e| invalid(e.to_string()))?;
        let input = encode_by_fields!(fact_json: String = fact_json).into();
        Ok(Some(CertifiedCallParams {
            origin: Address::ZERO,
            method: "update_certified_credit".to_string(),
            input,
            target: init.target,
        }))
    }
}

#[method::network(group = oracle::certified::credit_proof::two_phase)]
fn update_certified_credit(ctx: &mut _, fact_json: String) -> LyquidResult<bool> {
    let fact: CreditFact = serde_json::from_str(&fact_json)
        .map_err(|err| invalid(format!("invalid signed credit fact: {err}")))?;
    let user = fact
        .query
        .user
        .parse::<Address>()
        .map_err(|_| invalid("invalid credit fact user"))?;
    if fact
        != credit::credit_fact(
            user,
            fact.query.start_block,
            fact.query.end_block,
            fact.query.params,
        )?
    {
        return Err(invalid("signed credit fact does not match deterministic replay"));
    }
    if ctx.cert.signers.len() != ctx.cert.signatures.len() {
        return Err(invalid("runtime signer/signature count mismatch"));
    }
    let runtime_cert = ctx.cert.clone();
    let mut node_signatures = Vec::new();
    for (signer_id, signature) in runtime_cert.signers.iter().zip(&runtime_cert.signatures) {
        let node_id = ctx
            .signer_node_id(*signer_id as u64)
            .ok_or_else(|| invalid("runtime signer is not in active committee"))?;
        node_signatures.push(NodeSignature {
            node_id: node_id.to_string(),
            signer_id: *signer_id,
            signature: hex::encode(signature),
        });
    }
    let cert_hash = fact.query.hash();
    if let Some(existing) = ctx.network.credit_certificates.get(&cert_hash) {
        if existing.fact_hash != fact.hash() {
            return Err(invalid("conflicting credit certificate for query"));
        }
        *ctx.network.latest_credit_query_hash = cert_hash;
        return Ok(true);
    }
    let created_at = lyquor_api::chain_pos()?.block();
    let certificate = credit::build_credit_certificate(
        &fact,
        node_signatures,
        &runtime_cert,
        ctx.lyquid_id,
        created_at,
    )?;
    *ctx.network.latest_credit_query_hash = cert_hash.clone();
    ctx.network.credit_certificates.insert(cert_hash, certificate);
    Ok(true)
}

#[method::instance(export = eth)]
fn report_credit(ctx: &mut _) -> LyquidResult<bool> {
    let target = OracleTarget {
        seq_id: lyquor_api::sequence_backend_id()?,
        target: OracleServiceTarget::LVM(ctx.lyquid_id),
    };
    let oracle = ctx.network.credit_proof.clone();
    let user = *ctx.network.credit_query_user;
    let start_block = *ctx.network.credit_query_start;
    let end_block = *ctx.network.credit_query_end;
    let w1 = *ctx.network.credit_w1;
    let w2 = *ctx.network.credit_w2;
    let alpha = *ctx.network.credit_alpha;
    let wash_window = *ctx.network.credit_wash_window;
    let call = oracle.propose_and_certify(
        &mut ctx,
        target,
        encode_by_fields!(
            avg_num: u16 = COMMITTEE_THRESHOLD as u16,
            target: OracleTarget = target,
            user: Address = user,
            start_block: u64 = start_block,
            end_block: u64 = end_block,
            w1: u64 = w1,
            w2: u64 = w2,
            alpha: u64 = alpha,
            wash_window: u64 = wash_window
        )
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

#[method::network(export = eth)]
fn get_credit_certificate_json(
    ctx: &_,
    user: Address,
    start_block: u64,
    end_block: u64,
) -> LyquidResult<String> {
    let params = CreditParams {
        w1: *ctx.network.credit_w1,
        w2: *ctx.network.credit_w2,
        alpha: *ctx.network.credit_alpha,
        wash_window: *ctx.network.credit_wash_window,
    };
    let key = credit::make_credit_query(user, start_block, end_block, params).hash();
    let certificate = ctx
        .network
        .credit_certificates
        .get(&key)
        .ok_or_else(|| invalid("credit certificate not found"))?;
    serde_json::to_string(certificate)
        .map_err(|err| invalid(format!("credit certificate JSON error: {err}")))
}

#[method::network(export = eth)]
fn invalidate_credit_certificate(
    ctx: &mut _,
    user: Address,
    start_block: u64,
    end_block: u64,
) -> LyquidResult<bool> {
    if ctx.caller != *ctx.network.initializer {
        return Err(invalid("only initializer can invalidate credit certificate"));
    }
    let params = CreditParams {
        w1: *ctx.network.credit_w1,
        w2: *ctx.network.credit_w2,
        alpha: *ctx.network.credit_alpha,
        wash_window: *ctx.network.credit_wash_window,
    };
    let key = credit::make_credit_query(user, start_block, end_block, params).hash();
    let certificate = ctx
        .network
        .credit_certificates
        .get_mut(&key)
        .ok_or_else(|| invalid("credit certificate not found"))?;
    certificate.status = CertificateStatus::Invalidated;
    Ok(true)
}

#[method::network(export = eth)]
fn set_epoch_target(ctx: &mut _, to_block: u64) -> LyquidResult<bool> {
    let interval = *ctx.network.epoch_interval;
    if to_block == 0
        || to_block > holderproof_types::FIXTURE_END_BLOCK
        || to_block % interval != 0
    {
        return Err(invalid("epoch target must be a valid multiple of epoch_interval"));
    }
    *ctx.network.pending_epoch_block = to_block;
    Ok(true)
}

mod checkpoint_oracle {
    use super::*;

    #[method::instance(group = oracle::two_phase::checkpoint_proof)]
    fn propose(
        _ctx: &mut _,
        _avg_num: u16,
        _target: OracleTarget,
        to_block: u64,
        interval: u64,
    ) -> LyquidResult<EpochFact> {
        fixture::fixture_epoch_fact(to_block, interval)
    }

    #[method::instance(group = oracle::two_phase::checkpoint_proof)]
    fn aggregate(ctx: &_) -> LyquidResult<Option<CertifiedCallParams>> {
        let init = decode_by_fields!(
            ctx.init,
            avg_num: u16,
            target: OracleTarget,
            to_block: u64,
            interval: u64
        )
        .ok_or_else(|| invalid("invalid checkpoint committee init"))?;
        if init.target.seq_id != lyquor_api::sequence_backend_id()?
            || init.target.target != OracleServiceTarget::LVM(ctx.lyquid_id)
        {
            return Ok(None);
        }
        if init.avg_num as usize != COMMITTEE_THRESHOLD {
            return Err(invalid("unexpected checkpoint proposal threshold"));
        }
        let mut proposals: Vec<(_, EpochFact)> = Vec::new();
        for input in ctx.inputs {
            if let Some(p) = lyquor_primitives::decode_object::<EpochFact>(&input.input) {
                proposals.push((input.from, p));
            }
        }
        let Some(agreed) = agreeing_proposal(&proposals, COMMITTEE_THRESHOLD) else {
            return Ok(None);
        };
        if agreed.checkpoint.to_block != init.to_block || agreed.interval != init.interval {
            return Err(invalid("epoch fact does not match target block"));
        }
        let fact_json = serde_json::to_string(agreed).map_err(|e| invalid(e.to_string()))?;
        let input = encode_by_fields!(fact_json: String = fact_json).into();
        Ok(Some(CertifiedCallParams {
            origin: Address::ZERO,
            method: "update_certified_epoch_checkpoint".to_string(),
            input,
            target: init.target,
        }))
    }
}

#[method::network(group = oracle::certified::checkpoint_proof::two_phase)]
fn update_certified_epoch_checkpoint(ctx: &mut _, fact_json: String) -> LyquidResult<bool> {
    let fact: EpochFact = serde_json::from_str(&fact_json)
        .map_err(|err| invalid(format!("invalid epoch fact: {err}")))?;
    let to_block = fact.checkpoint.to_block;
    if fact != fixture::fixture_epoch_fact(to_block, fact.interval)? {
        return Err(invalid("signed epoch fact does not match deterministic replay"));
    }
    if let Some(existing) = ctx.network.epoch_checkpoints.get(&to_block) {
        if existing.fact_hash != fact.hash() {
            return Err(invalid("conflicting epoch checkpoint for block"));
        }
        return Ok(true);
    }
    let runtime_cert = ctx.cert.clone();
    if runtime_cert.signers.len() < COMMITTEE_THRESHOLD
        || runtime_cert.signers.len() != runtime_cert.signatures.len()
    {
        return Err(invalid("invalid epoch runtime signatures"));
    }
    let mut node_signatures = Vec::new();
    let mut signer_ids = Vec::new();
    for (signer_id, sig) in runtime_cert.signers.iter().zip(&runtime_cert.signatures) {
        let node_id = ctx
            .signer_node_id(*signer_id as u64)
            .ok_or_else(|| invalid("epoch signer not in active committee"))?
            .to_string();
        if signer_ids.contains(&node_id) {
            return Err(invalid("duplicate epoch signer"));
        }
        signer_ids.push(node_id.clone());
        node_signatures.push(NodeSignature {
            node_id,
            signer_id: *signer_id,
            signature: hex::encode(sig),
        });
    }
    let created_at = lyquor_api::chain_pos()?.block();
    let runtime_certificate = serde_json::to_string(&runtime_cert)
        .map_err(|err| invalid(format!("epoch cert JSON error: {err}")))?;
    let ep = EpochCheckpoint {
        epoch_id: fact.epoch_id,
        interval: fact.interval,
        fact_hash: fact.hash(),
        checkpoint: fact.checkpoint,
        total_transfer_volume: fact.total_transfer_volume,
        unique_holders: fact.unique_holders,
        signer_ids,
        node_signatures,
        runtime_certificate,
        network_state_id: format!("{}@{}", ctx.lyquid_id, created_at),
        status: CertificateStatus::Finalized,
        created_at,
    };
    if to_block > *ctx.network.latest_epoch_block {
        *ctx.network.latest_epoch_block = to_block;
    }
    ctx.network.epoch_checkpoints.insert(to_block, ep);
    Ok(true)
}

#[method::instance(export = eth)]
fn report_epoch_checkpoint(ctx: &mut _) -> LyquidResult<bool> {
    let target = OracleTarget {
        seq_id: lyquor_api::sequence_backend_id()?,
        target: OracleServiceTarget::LVM(ctx.lyquid_id),
    };
    let oracle = ctx.network.checkpoint_proof.clone();
    let to_block = *ctx.network.pending_epoch_block;
    let interval = *ctx.network.epoch_interval;
    let call = oracle.propose_and_certify(
        &mut ctx,
        target,
        encode_by_fields!(
            avg_num: u16 = COMMITTEE_THRESHOLD as u16,
            target: OracleTarget = target,
            to_block: u64 = to_block,
            interval: u64 = interval
        )
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

#[method::network(export = eth)]
fn get_epoch_checkpoint_json(ctx: &_, to_block: u64) -> LyquidResult<String> {
    let ep = ctx
        .network
        .epoch_checkpoints
        .get(&to_block)
        .ok_or_else(|| invalid("epoch checkpoint not found"))?;
    serde_json::to_string(ep).map_err(|e| invalid(e.to_string()))
}

#[method::network(export = eth)]
fn set_sepolia_query(ctx: &mut _, holder: Address) -> LyquidResult<bool> {
    if holder == Address::ZERO {
        return Err(invalid("sepolia holder cannot be zero"));
    }
    *ctx.network.sepolia_query_holder = holder;
    Ok(true)
}

mod sepolia_oracle {
    use super::*;

    #[method::instance(group = oracle::two_phase::sepolia_proof)]
    fn propose(
        _ctx: &mut _,
        _avg_num: u16,
        _target: OracleTarget,
        holder: Address,
    ) -> LyquidResult<BalanceProposal> {
        sepolia::sepolia_fact(holder)
    }

    #[method::instance(group = oracle::two_phase::sepolia_proof)]
    fn aggregate(ctx: &_) -> LyquidResult<Option<CertifiedCallParams>> {
        let init = decode_by_fields!(ctx.init, avg_num: u16, target: OracleTarget, holder: Address)
            .ok_or_else(|| invalid("invalid sepolia committee init"))?;
        if init.target.seq_id != lyquor_api::sequence_backend_id()?
            || init.target.target != OracleServiceTarget::LVM(ctx.lyquid_id)
        {
            return Ok(None);
        }
        if init.avg_num as usize != COMMITTEE_THRESHOLD {
            return Err(invalid("unexpected sepolia proposal threshold"));
        }
        let mut proposals: Vec<(_, BalanceProposal)> = Vec::new();
        for input in ctx.inputs {
            if let Some(p) = lyquor_primitives::decode_object::<BalanceProposal>(&input.input) {
                proposals.push((input.from, p));
            }
        }
        let Some(agreed) = agreeing_proposal(&proposals, COMMITTEE_THRESHOLD) else {
            return Ok(None);
        };
        if agreed.query.holder != init.holder.to_string().to_ascii_lowercase() {
            return Err(invalid("sepolia fact holder mismatch"));
        }
        let fact_json = serde_json::to_string(agreed).map_err(|e| invalid(e.to_string()))?;
        let input = encode_by_fields!(fact_json: String = fact_json).into();
        Ok(Some(CertifiedCallParams {
            origin: Address::ZERO,
            method: "update_certified_sepolia".to_string(),
            input,
            target: init.target,
        }))
    }
}

#[method::network(group = oracle::certified::sepolia_proof::two_phase)]
fn update_certified_sepolia(ctx: &mut _, fact_json: String) -> LyquidResult<bool> {
    let fact: holderproof_types::BalanceFact = serde_json::from_str(&fact_json)
        .map_err(|err| invalid(format!("invalid sepolia fact: {err}")))?;
    let holder = fact
        .query
        .holder
        .parse::<Address>()
        .map_err(|_| invalid("invalid sepolia fact holder"))?;
    if fact != sepolia::sepolia_fact(holder)? {
        return Err(invalid("signed sepolia fact does not match deterministic replay"));
    }
    let cert_hash = fact.query.hash();
    if let Some(existing) = ctx.network.sepolia_certificates.get(&cert_hash) {
        if existing.fact_hash != fact.hash() {
            return Err(invalid("conflicting sepolia certificate"));
        }
        *ctx.network.latest_sepolia_query_hash = cert_hash;
        return Ok(true);
    }
    let runtime_cert = ctx.cert.clone();
    if runtime_cert.signers.len() != runtime_cert.signatures.len() {
        return Err(invalid("sepolia signer/signature count mismatch"));
    }
    let mut node_signatures = Vec::new();
    for (signer_id, sig) in runtime_cert.signers.iter().zip(&runtime_cert.signatures) {
        let node_id = ctx
            .signer_node_id(*signer_id as u64)
            .ok_or_else(|| invalid("sepolia signer not in active committee"))?;
        node_signatures.push(NodeSignature {
            node_id: node_id.to_string(),
            signer_id: *signer_id,
            signature: hex::encode(sig),
        });
    }
    let created_at = lyquor_api::chain_pos()?.block();
    let cert = certificate_builder::build(
        &fact,
        node_signatures,
        &runtime_cert,
        ctx.lyquid_id,
        created_at,
    )?;
    *ctx.network.latest_sepolia_query_hash = cert_hash.clone();
    ctx.network.sepolia_certificates.insert(cert_hash, cert);
    Ok(true)
}

#[method::instance(export = eth)]
fn report_sepolia_balance(ctx: &mut _) -> LyquidResult<bool> {
    let target = OracleTarget {
        seq_id: lyquor_api::sequence_backend_id()?,
        target: OracleServiceTarget::LVM(ctx.lyquid_id),
    };
    let oracle = ctx.network.sepolia_proof.clone();
    let holder = *ctx.network.sepolia_query_holder;
    let call = oracle.propose_and_certify(
        &mut ctx,
        target,
        encode_by_fields!(
            avg_num: u16 = COMMITTEE_THRESHOLD as u16,
            target: OracleTarget = target,
            holder: Address = holder
        )
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

#[method::network(export = eth)]
fn get_sepolia_certificate_json(ctx: &_, holder: Address) -> LyquidResult<String> {
    let key = sepolia::sepolia_query(holder)?.hash();
    let cert = ctx
        .network
        .sepolia_certificates
        .get(&key)
        .ok_or_else(|| invalid("sepolia certificate not found"))?;
    serde_json::to_string(cert).map_err(|e| invalid(e.to_string()))
}