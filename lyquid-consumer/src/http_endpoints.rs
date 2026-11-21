//! HTTP diagnostics for the HolderProof Consumer.

use super::*;

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

#[method::instance(export = http, method = "GET", path_prefix = "/api/result")]
fn http_result(ctx: &_, _req: http::Request) -> LyquidResult<http::Response> {
    let body = serde_json::to_vec(&serde_json::json!({
        "claimCount": *ctx.network.claim_count,
        "role": "AirdropGateLyquid",
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
    .map_err(|err| invalid(format!("result JSON error: {err}")))?;
    Ok(json_response(body))
}

#[method::instance(export = http, method = "GET", path_prefix = "/api/history")]
fn http_history(ctx: &_, _req: http::Request) -> LyquidResult<http::Response> {
    let history = ctx.instance.result_history.read();
    let body = serde_json::to_vec(&*history).map_err(|err| invalid(err.to_string()))?;
    Ok(json_response(body))
}
