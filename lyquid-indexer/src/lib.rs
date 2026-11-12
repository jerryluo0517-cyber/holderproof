//! HolderProof Indexer Lyquid.
//!
//! Reading order:
//! 1. `state!` below shows shared network state.
//! 2. `set_query` selects the holder and historical block.
//! 3. `propose` computes the fixture balance on every committee node.
//! 4. `aggregate` requires at least three matching proposals.
//! 5. `update_certified_balance` stores the full certificate.
//! 6. `get_certificate_json` exposes the certificate to the Consumer.
//! 7. `report_balance` starts the certified-call workflow.
//!
//! Pure data and algorithms live outside this file:
//! - `fixture.rs`: fixture parsing, balance and TWAB
//! - `merkle.rs`: event leaves and Merkle root
//! - `models.rs`: committee proposal
//! - `holderproof-types`: shared certificate wire types

use fixture::{
    fixture_balance, fixture_checkpoint, fixture_token, fixture_twab, make_query_spec, query_hash,
};
use holderproof_types::{CHAIN_ID, COMMITTEE_THRESHOLD, CertificateStatus, QueryCertificate};
use lyquid::prelude::*;
use models::BalanceProposal;

/// Build a runtime error without the `LyquidError::LyquorRuntime(...into())`
/// boilerplate.
pub(crate) fn invalid(reason: impl Into<String>) -> LyquidError {
    LyquidError::LyquorRuntime(reason.into())
}

mod fixture;
mod http_endpoints;
mod merkle;
mod models;
#[cfg(test)]
mod tests;

state! {
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

// Read the balance fact consumed by the consumer Lyquid.
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
    if holder == *ctx.network.certified_holder
        && block == *ctx.network.certified_block
        && *ctx.network.certified_signer_count >= COMMITTEE_THRESHOLD as u64
    {
        Ok(*ctx.network.certified_balance)
    } else {
        Err(invalid("HolderProof certified balance not available"))
    }
}

// Configure the 4-node oracle committee. Threshold = n / 2 + 1 = 3.
#[method::network(export = eth)]
fn configure_committee(ctx: &mut _, node_ids: Vec<NodeID>) -> LyquidResult<bool> {
    if ctx.caller != *ctx.network.initializer {
        return Err(invalid("only initializer can configure committee"));
    }
    if node_ids.is_empty() {
        return Err(invalid("committee cannot be empty"));
    }

    let target = OracleTarget {
        seq_id: lyquor_api::sequence_backend_id()?,
        target: OracleServiceTarget::LVM(ctx.lyquid_id),
    };
    // n / 2 + 1: equals COMMITTEE_THRESHOLD (3) for the 4-node devnet.
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
fn propose(ctx: &mut _, _avg_num: u16, _target: OracleTarget) -> LyquidResult<BalanceProposal> {
    let holder = *ctx.network.query_holder;
    let block = *ctx.network.query_block;
    if holder == Address::ZERO {
        return Err(invalid("query is not configured"));
    }
    let balance = fixture_balance(holder, block)?;
    let proposal = BalanceProposal {
        holder,
        block,
        balance,
    };
    lyquid::println!("holderproof-indexer: fixture proposal = {:?}", proposal);
    Ok(proposal)
}

#[method::instance(group = oracle::two_phase::balance_proof)]
fn aggregate(ctx: &_) -> LyquidResult<Option<CertifiedCallParams>> {
    let init = decode_by_fields!(ctx.init, avg_num: u16, target: OracleTarget)
        .ok_or_else(|| invalid("invalid balance committee init"))?;
    if init.target.seq_id != lyquor_api::sequence_backend_id()? {
        return Ok(None);
    }

    let mut proposals = Vec::new();
    for input in ctx.inputs {
        if let Some(proposal) = lyquor_primitives::decode_object::<BalanceProposal>(&input.input) {
            proposals.push((input.from, proposal));
        }
    }
    if proposals.len() < init.avg_num as usize {
        return Ok(None);
    }

    let (_, first) = &proposals[0];
    let matching = proposals
        .iter()
        .filter(|(_, proposal)| {
            proposal.holder == first.holder
                && proposal.block == first.block
                && proposal.balance == first.balance
        })
        .count();
    if matching < init.avg_num as usize {
        lyquid::println!(
            "holderproof-indexer: no committee agreement ({}/{} required)",
            matching,
            init.avg_num
        );
        return Ok(None);
    }

    let input = encode_by_fields!(
        holder: Address = first.holder,
        block: u64 = first.block,
        balance: U256 = first.balance
    )
    .into();
    Ok(Some(CertifiedCallParams {
        origin: Address::ZERO,
        method: "update_certified_balance".to_string(),
        input,
        target: init.target,
    }))
}

#[method::network(group = oracle::certified::balance_proof::two_phase)]
fn update_certified_balance(
    ctx: &mut _,
    holder: Address,
    block: u64,
    balance: U256,
) -> LyquidResult<bool> {
    let signer_ids: Vec<String> = ctx
        .cert
        .signers
        .clone()
        .into_iter()
        .filter_map(|signer_id| ctx.signer_node_id(signer_id as u64))
        .map(|node_id| node_id.to_string())
        .collect();
    let signer_count = signer_ids.len() as u64;
    let token = fixture_token()?;
    let query = make_query_spec(&token, holder, block);
    let certificate_hash = query_hash(&query);
    let checkpoint = fixture_checkpoint(block)?;
    let certificate = QueryCertificate {
        certificate_id: certificate_hash.clone(),
        query_hash: certificate_hash.clone(),
        chain_id: CHAIN_ID,
        token,
        holder: holder.to_string().to_ascii_lowercase(),
        block_number: block,
        balance: balance.to_string(),
        checkpoint,
        signer_ids,
        status: CertificateStatus::Finalized,
        created_at: lyquor_api::chain_pos()?.block(),
    };

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
    let call = oracle.propose_and_certify(
        &mut ctx,
        target,
        encode_by_fields!(avg_num: u16 = COMMITTEE_THRESHOLD as u16, target: OracleTarget = target)
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
