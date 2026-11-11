//! HolderProof indexer Lyquid.
//!
//! Step 2 starts with the minimum shared fact needed by the consumer:
//! a holder's historical balance at one block. The committee certification
//! flow will replace `publish_balance` once the cross-Lyquid call is proven.

use lyquid::prelude::*;
use serde::{Deserialize, Serialize};

const FIXTURE_JSON: &str = include_str!("../../fixtures/transfers.json");

#[derive(Debug, Clone, Deserialize)]
struct TransferEvent {
    block_number: u64,
    log_index: u64,
    from: String,
    to: String,
    amount: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BalanceProposal {
    holder: Address,
    block: u64,
    balance: U256,
}

fn fixture_events() -> LyquidResult<Vec<TransferEvent>> {
    serde_json::from_str(FIXTURE_JSON)
        .map_err(|err| LyquidError::LyquorRuntime(format!("fixture JSON error: {err}")))
}

fn fixture_balance(holder: Address, block: u64) -> LyquidResult<U256> {
    let mut events = fixture_events()?;
    events.sort_by_key(|event| (event.block_number, event.log_index));
    let holder_hex = holder.to_string().to_ascii_lowercase();
    let mut balance = U256::ZERO;
    for event in events {
        if event.block_number > block {
            break;
        }
        let amount = event.amount.parse::<U256>().map_err(|_| {
            LyquidError::LyquorRuntime(format!("invalid fixture amount: {}", event.amount))
        })?;
        if event.to.to_ascii_lowercase() == holder_hex {
            balance += amount;
        }
        if event.from.to_ascii_lowercase() == holder_hex {
            balance -= amount;
        }
    }
    Ok(balance)
}

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
}

#[method::network(export = eth)]
fn constructor(ctx: &mut _) {
    *ctx.network.initializer = ctx.caller;
}

#[method::network(export = eth)]
fn set_query(ctx: &mut _, holder: Address, block: u64) -> LyquidResult<bool> {
    if holder == Address::ZERO {
        return Err(LyquidError::LyquorRuntime(
            "holder cannot be the zero address".into(),
        ));
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
        Err(LyquidError::LyquidRuntime(
            "HolderProof snapshot not found".into(),
        ))
    }
}

// Consumer-facing read of the committee-certified result.
#[method::network(export = eth)]
fn get_certified_balance(ctx: &_, holder: Address, block: u64) -> LyquidResult<U256> {
    if holder == *ctx.network.certified_holder
        && block == *ctx.network.certified_block
        && *ctx.network.certified_signer_count >= 3
    {
        Ok(*ctx.network.certified_balance)
    } else {
        Err(LyquidError::LyquidRuntime(
            "HolderProof certified balance not available".into(),
        ))
    }
}

// HTTP diagnostics make the cross-Lyquid test observable without relying on an
// Ethereum static call to an off-chain Lyquid instance.
#[method::instance(export = http, method = "GET", path_prefix = "/api/balance")]
fn http_balance(ctx: &_, _req: http::Request) -> LyquidResult<http::Response> {
    let body = serde_json::to_vec(&serde_json::json!({
        "holder": ctx.network.snapshot_holder.to_string(),
        "block": *ctx.network.snapshot_block,
        "balance": ctx.network.snapshot_balance.to_string(),
    }))
    .map_err(|err| LyquidError::LyquorRuntime(format!("balance JSON error: {err}")))?;

    Ok(http::Response {
        status: 200,
        headers: vec![http::Header {
            name: "content-type".into(),
            value: b"application/json; charset=utf-8".to_vec(),
        }],
        body,
    })
}

// Configure the 4-node oracle committee. Threshold = n / 2 + 1 = 3.
#[method::network(export = eth)]
fn configure_committee(ctx: &mut _, node_ids: Vec<NodeID>) -> LyquidResult<bool> {
    if ctx.caller != *ctx.network.initializer {
        return Err(LyquidError::LyquorRuntime(
            "only initializer can configure committee".into(),
        ));
    }
    if node_ids.is_empty() {
        return Err(LyquidError::LyquorRuntime(
            "committee cannot be empty".into(),
        ));
    }

    let target = OracleTarget {
        seq_id: lyquor_api::sequence_backend_id()?,
        target: OracleServiceTarget::LVM(ctx.lyquid_id),
    };
    let threshold = u16::try_from(node_ids.len() / 2 + 1)
        .map_err(|_| LyquidError::LyquorRuntime("committee threshold overflow".into()))?;

    if !ctx
        .network
        .balance_proof
        .clone()
        .initialize(&mut ctx, target, node_ids, threshold)
    {
        return Err(LyquidError::LyquorRuntime(
            "balance committee initialization failed".into(),
        ));
    }
    Ok(true)
}

// Each committee node independently replays the embedded Transfer fixture.
#[method::instance(group = oracle::two_phase::balance_proof)]
fn propose(ctx: &mut _, _avg_num: u16, _target: OracleTarget) -> LyquidResult<BalanceProposal> {
    let holder = *ctx.network.query_holder;
    let block = *ctx.network.query_block;
    if holder == Address::ZERO {
        return Err(LyquidError::LyquorRuntime("query is not configured".into()));
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
        .ok_or_else(|| LyquidError::LyquorRuntime("invalid balance committee init".into()))?;
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
        method: "update_certified_balance".into(),
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
    let signer_count = ctx.cert.signers.len() as u64;
    *ctx.network.certified_holder = holder;
    *ctx.network.certified_block = block;
    *ctx.network.certified_balance = balance;
    *ctx.network.certified_signer_count = signer_count;
    lyquid::println!(
        "holderproof-indexer: certified balance {:?} for {:?} at block {} by {} signers",
        balance,
        holder,
        block,
        signer_count
    );
    Ok(true)
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
        encode_by_fields!(avg_num: u16 = 3, target: OracleTarget = target).into(),
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

#[method::instance(export = http, method = "GET", path_prefix = "/api/certified")]
fn http_certified(ctx: &_, _req: http::Request) -> LyquidResult<http::Response> {
    let body = serde_json::to_vec(&serde_json::json!({
        "holder": ctx.network.certified_holder.to_string(),
        "block": *ctx.network.certified_block,
        "balance": ctx.network.certified_balance.to_string(),
        "signerCount": *ctx.network.certified_signer_count,
    }))
    .map_err(|err| LyquidError::LyquorRuntime(format!("certified JSON error: {err}")))?;

    Ok(http::Response {
        status: 200,
        headers: vec![http::Header {
            name: "content-type".into(),
            value: b"application/json; charset=utf-8".to_vec(),
        }],
        body,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use lyquid_test::test;

    #[test]
    fn fixture_has_expected_event_count() {
        assert_eq!(fixture_events().unwrap().len(), 308);
    }

    #[test]
    fn alice_snapshots_match_python_ground_truth() {
        let alice = Address::new([0xaa; 20]);
        assert_eq!(fixture_balance(alice, 900).unwrap(), U256::from(47_685u64));
        assert_eq!(fixture_balance(alice, 950).unwrap(), U256::from(57_685u64));
        assert_eq!(fixture_balance(alice, 1000).unwrap(), U256::from(62_685u64));
    }

    #[test]
    fn unknown_holder_is_zero() {
        let unknown = Address::new([0x99; 20]);
        assert_eq!(fixture_balance(unknown, 1000).unwrap(), U256::ZERO);
    }
}
