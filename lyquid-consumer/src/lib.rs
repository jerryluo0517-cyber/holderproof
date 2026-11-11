use lyquid::prelude::*;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct CertificateCheckpoint {
    from_block: u64,
    to_block: u64,
    block_hash: String,
    event_count: u64,
    event_merkle_root: String,
    parser_version: String,
}

#[derive(Debug, Deserialize)]
struct CertificateView {
    certificate_id: String,
    query_hash: String,
    chain_id: u64,
    token: String,
    holder: String,
    block_number: u64,
    balance: String,
    checkpoint: CertificateCheckpoint,
    signer_ids: Vec<String>,
    status: String,
    created_at: u64,
}

state! {
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
fn constructor(_ctx: &mut _) {}

#[method::network(export = eth)]
fn configure(
    ctx: &mut _,
    indexer: Address,
    expected_holder: Address,
    expected_block: u64,
    threshold: U256,
) -> LyquidResult<bool> {
    *ctx.network.indexer = indexer;
    *ctx.network.expected_holder = expected_holder;
    *ctx.network.expected_block = expected_block;
    *ctx.network.threshold = threshold;
    Ok(true)
}

#[method::network(export = eth)]
fn check_eligible(ctx: &mut _) -> LyquidResult<bool> {
    let indexer_address = *ctx.network.indexer;
    let holder = *ctx.network.expected_holder;
    let block = *ctx.network.expected_block;
    let threshold = *ctx.network.threshold;
    if indexer_address == Address::ZERO {
        return Err(LyquidError::LyquorRuntime(
            "HolderProof consumer indexer is not configured".into(),
        ));
    }
    let indexer = LyquidID::from(indexer_address);
    let response = call!(
        (indexer).get_certificate_json(
            holder: Address = holder,
            block: u64 = block
        ) -> (certificate: LyquidResult<String>)
    );
    let certificate: CertificateView = serde_json::from_str(&response.certificate?)
        .map_err(|err| LyquidError::LyquorRuntime(format!("certificate decode error: {err}")))?;

    if certificate.status != "Finalized" {
        return Err(LyquidError::LyquorRuntime(
            "certificate is not finalized".into(),
        ));
    }
    if certificate.signer_ids.len() < 3 {
        return Err(LyquidError::LyquorRuntime(
            "certificate does not have 3 signers".into(),
        ));
    }
    if certificate.block_number != block
        || certificate.holder.to_ascii_lowercase() != holder.to_string().to_ascii_lowercase()
    {
        return Err(LyquidError::LyquorRuntime(
            "certificate query does not match request".into(),
        ));
    }
    if certificate.chain_id != 11_155_111
        || certificate.token.is_empty()
        || certificate.created_at == 0
        || certificate.query_hash.is_empty()
        || certificate.checkpoint.from_block > certificate.checkpoint.to_block
        || certificate.checkpoint.to_block != block
        || certificate.checkpoint.block_hash.is_empty()
        || certificate.checkpoint.event_count == 0
        || certificate.checkpoint.event_merkle_root.is_empty()
        || certificate.checkpoint.parser_version.is_empty()
        || certificate.certificate_id.is_empty()
    {
        return Err(LyquidError::LyquorRuntime(
            "certificate is missing or has invalid required fields".into(),
        ));
    }

    let balance = certificate
        .balance
        .parse::<U256>()
        .map_err(|_| LyquidError::LyquorRuntime("invalid certificate balance".into()))?;
    let eligible = balance >= threshold;
    *ctx.network.last_balance = balance;
    *ctx.network.last_eligible = eligible;
    *ctx.network.check_count += 1;
    *ctx.network.last_query_hash = certificate.query_hash;
    *ctx.network.last_block_hash = certificate.checkpoint.block_hash;
    *ctx.network.last_event_merkle_root = certificate.checkpoint.event_merkle_root;
    *ctx.network.last_certificate_status = certificate.status;
    *ctx.network.last_signer_count = certificate.signer_ids.len() as u64;
    Ok(eligible)
}

#[method::instance(export = http, method = "GET", path_prefix = "/api/result")]
fn http_result(ctx: &_, _req: http::Request) -> LyquidResult<http::Response> {
    let body = serde_json::to_vec(&serde_json::json!({
        "indexer": ctx.network.indexer.to_string(),
        "holder": ctx.network.expected_holder.to_string(),
        "block": *ctx.network.expected_block,
        "threshold": ctx.network.threshold.to_string(),
        "balance": ctx.network.last_balance.to_string(),
        "eligible": *ctx.network.last_eligible,
        "checkCount": *ctx.network.check_count,
        "certificateStatus": ctx.network.last_certificate_status.to_string(),
        "signerCount": *ctx.network.last_signer_count,
        "queryHash": ctx.network.last_query_hash.to_string(),
        "blockHash": ctx.network.last_block_hash.to_string(),
        "eventMerkleRoot": ctx.network.last_event_merkle_root.to_string(),
    }))
    .map_err(|err| LyquidError::LyquorRuntime(format!("result JSON error: {err}")))?;
    Ok(http::Response {
        status: 200,
        headers: vec![http::Header {
            name: "content-type".into(),
            value: b"application/json; charset=utf-8".to_vec(),
        }],
        body,
    })
}
