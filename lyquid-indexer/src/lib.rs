//! HolderProof indexer Lyquid.
//!
//! Step 2 starts with the minimum shared fact needed by the consumer:
//! a holder's historical balance at one block. The committee certification
//! flow will replace `publish_balance` once the cross-Lyquid call is proven.

use lyquid::prelude::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const FIXTURE_JSON: &str = include_str!("../../fixtures/transfers.json");
const PARSER_VERSION: &str = "holderproof-fixture-v1";
const CHAIN_ID: u64 = 11_155_111;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TransferEvent {
    chain_id: u64,
    token: String,
    block_number: u64,
    block_hash: String,
    tx_hash: String,
    log_index: u64,
    from: String,
    to: String,
    amount: String,
    parser_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BalanceProposal {
    holder: Address,
    block: u64,
    balance: U256,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct QuerySpec {
    chain_id: u64,
    token: String,
    holder: String,
    block: u64,
    parser_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Checkpoint {
    from_block: u64,
    to_block: u64,
    block_hash: String,
    event_count: u64,
    event_merkle_root: String,
    parser_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct QueryCertificate {
    certificate_id: String,
    query_hash: String,
    chain_id: u64,
    token: String,
    holder: String,
    block_number: u64,
    balance: String,
    checkpoint: Checkpoint,
    signer_ids: Vec<String>,
    status: String,
    created_at: u64,
}

fn hash_hex(data: &[u8]) -> String {
    use std::fmt::Write as _;
    let digest = Sha256::digest(data);
    let mut out = String::with_capacity(64);
    for byte in digest {
        write!(&mut out, "{byte:02x}").expect("writing to String cannot fail");
    }
    out
}

fn event_leaf(event: &TransferEvent) -> String {
    hash_hex(
        format!(
            "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
            event.chain_id,
            event.token,
            event.block_number,
            event.block_hash,
            event.tx_hash,
            event.log_index,
            event.from,
            event.to,
            event.amount,
            event.parser_version
        )
        .as_bytes(),
    )
}

fn merkle_root(leaves: &[String]) -> LyquidResult<String> {
    if leaves.is_empty() {
        return Err(LyquidError::LyquorRuntime(
            "cannot build Merkle root from empty events".into(),
        ));
    }
    let mut layer = leaves.to_vec();
    while layer.len() > 1 {
        if layer.len() % 2 == 1 {
            layer.push(layer.last().expect("non-empty layer").clone());
        }
        let mut next = Vec::with_capacity(layer.len() / 2);
        for pair in layer.chunks(2) {
            let mut bytes = hex::decode(&pair[0])
                .map_err(|_| LyquidError::LyquorRuntime("invalid Merkle hex".into()))?;
            bytes.extend(
                hex::decode(&pair[1])
                    .map_err(|_| LyquidError::LyquorRuntime("invalid Merkle hex".into()))?,
            );
            next.push(hash_hex(&bytes));
        }
        layer = next;
    }
    Ok(layer.remove(0))
}

fn fixture_token() -> LyquidResult<String> {
    fixture_events()?
        .into_iter()
        .next()
        .map(|event| event.token)
        .ok_or_else(|| LyquidError::LyquorRuntime("fixture has no events".into()))
}

fn fixture_checkpoint(block: u64) -> LyquidResult<Checkpoint> {
    let mut events = fixture_events()?;
    events.sort_by_key(|event| (event.block_number, event.log_index));
    events.retain(|event| event.block_number <= block);
    let leaves: Vec<String> = events.iter().map(event_leaf).collect();
    Ok(Checkpoint {
        from_block: 0,
        to_block: block,
        block_hash: fixture_block_hash(block),
        event_count: events.len() as u64,
        event_merkle_root: merkle_root(&leaves)?,
        parser_version: PARSER_VERSION.to_string(),
    })
}

fn fixture_block_hash(block: u64) -> String {
    format!(
        "0x{}",
        hash_hex(format!("block|{CHAIN_ID}|{block}").as_bytes())
    )
}

fn make_query_spec(token: &str, holder: Address, block: u64) -> QuerySpec {
    QuerySpec {
        chain_id: CHAIN_ID,
        token: token.to_string(),
        holder: holder.to_string().to_ascii_lowercase(),
        block,
        parser_version: PARSER_VERSION.to_string(),
    }
}

fn query_hash(spec: &QuerySpec) -> String {
    hash_hex(
        format!(
            "query|{}|{}|{}|{}|{}",
            spec.chain_id, spec.token, spec.holder, spec.block, spec.parser_version
        )
        .as_bytes(),
    )
}

fn fixture_twab(holder: Address, start: u64, end: u64, scale: u64) -> LyquidResult<U256> {
    if end <= start {
        return Err(LyquidError::LyquorRuntime(
            "TWAB end block must be greater than start block".into(),
        ));
    }
    let mut events = fixture_events()?;
    events.sort_by_key(|event| (event.block_number, event.log_index));
    let holder_hex = holder.to_string().to_ascii_lowercase();
    let mut current = U256::ZERO;
    let mut total = U256::ZERO;
    let mut index = 0usize;
    for block_number in start..end {
        while index < events.len() && events[index].block_number <= block_number {
            let event = &events[index];
            let amount = event.amount.parse::<U256>().map_err(|_| {
                LyquidError::LyquorRuntime(format!("invalid fixture amount: {}", event.amount))
            })?;
            if event.to.to_ascii_lowercase() == holder_hex {
                current += amount;
            }
            if event.from.to_ascii_lowercase() == holder_hex {
                current -= amount;
            }
            index += 1;
        }
        total += current;
    }
    Ok(total * U256::from(scale) / U256::from(end - start))
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
        status: "Finalized".into(),
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
        .ok_or_else(|| LyquidError::LyquorRuntime("certificate not found".into()))?;
    serde_json::to_string(certificate)
        .map_err(|err| LyquidError::LyquorRuntime(format!("certificate JSON error: {err}")))
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
        "queryHash": ctx.network.latest_query_hash.to_string(),
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

#[method::instance(export = http, method = "GET", path_prefix = "/api/certificate")]
fn http_certificate(ctx: &_, _req: http::Request) -> LyquidResult<http::Response> {
    let certificate_hash = ctx.network.latest_query_hash.to_string();
    let certificate = ctx
        .network
        .certificates
        .get(&certificate_hash)
        .ok_or_else(|| LyquidError::LyquorRuntime("certificate not found".into()))?;
    let body = serde_json::to_vec(certificate)
        .map_err(|err| LyquidError::LyquorRuntime(format!("certificate JSON error: {err}")))?;

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
    #[test]
    fn merkle_root_matches_python_ground_truth() {
        let checkpoint = fixture_checkpoint(1000).unwrap();
        assert_eq!(checkpoint.event_count, 308);
        assert_eq!(
            checkpoint.event_merkle_root,
            "2e89babbc7b45637458bd08a2d843087354bf175eb559e5c1ddc7fcc0e79617b"
        );
    }

    #[test]
    fn twab_matches_python_ground_truth() {
        let alice = Address::new([0xaa; 20]);
        let bob = Address::new([0xbb; 20]);
        assert_eq!(
            fixture_twab(alice, 0, 1000, 1_000_000).unwrap(),
            U256::from(48_945_825_000u64)
        );
        assert_eq!(
            fixture_twab(bob, 0, 1000, 1_000_000).unwrap(),
            U256::from(47_662_030_000u64)
        );
    }
}
