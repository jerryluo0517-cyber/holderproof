//! HTTP diagnostics endpoints for the Indexer Lyquid.
//!
//! These make the cross-Lyquid flow observable without relying on an Ethereum
//! static call. They are registered via `#[method::instance(export = http)]`.
//!
//! `use super::*` brings in the `state!`-generated context type so the
//! handlers can read `ctx.network.*`, plus the `invalid` error helper.

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

#[method::instance(export = http, method = "GET", path_prefix = "/api/balance")]
fn http_balance(ctx: &_, _req: http::Request) -> LyquidResult<http::Response> {
    let body = serde_json::to_vec(&serde_json::json!({
        "holder": ctx.network.snapshot_holder.to_string(),
        "block": *ctx.network.snapshot_block,
        "balance": ctx.network.snapshot_balance.to_string(),
    }))
    .map_err(|err| invalid(format!("balance JSON error: {err}")))?;

    Ok(json_response(body))
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
    .map_err(|err| invalid(format!("certified JSON error: {err}")))?;

    Ok(json_response(body))
}

#[method::instance(export = http, method = "GET", path_prefix = "/api/certificate")]
fn http_certificate(ctx: &_, _req: http::Request) -> LyquidResult<http::Response> {
    let certificate_hash = ctx.network.latest_query_hash.to_string();
    let certificate = ctx
        .network
        .certificates
        .get(&certificate_hash)
        .ok_or_else(|| invalid("certificate not found"))?;
    let body = serde_json::to_vec(certificate)
        .map_err(|err| invalid(format!("certificate JSON error: {err}")))?;

    Ok(json_response(body))
}

#[method::instance(export = http, method = "GET", path_prefix = "/api/credit-certificate")]
fn http_credit_certificate(ctx: &_, _req: http::Request) -> LyquidResult<http::Response> {
    let key = ctx.network.latest_credit_query_hash.to_string();
    let certificate = ctx
        .network
        .credit_certificates
        .get(&key)
        .ok_or_else(|| invalid("credit certificate not found"))?;
    let body = serde_json::to_vec(certificate)
        .map_err(|err| invalid(format!("credit certificate JSON error: {err}")))?;
    Ok(json_response(body))
}

#[method::instance(export = http, method = "GET", path_prefix = "/api/epoch-checkpoints")]
fn http_epoch_checkpoints(ctx: &_, _req: http::Request) -> LyquidResult<http::Response> {
    let mut list: Vec<_> = ctx.network.epoch_checkpoints.values().cloned().collect();
    list.sort_by_key(|ep| ep.checkpoint.to_block);
    let body = serde_json::to_vec(&serde_json::json!({
        "epochInterval": *ctx.network.epoch_interval,
        "latestEpochBlock": *ctx.network.latest_epoch_block,
        "checkpoints": list,
    }))
    .map_err(|err| invalid(err.to_string()))?;
    Ok(json_response(body))
}

#[method::instance(export = http, method = "GET", path_prefix = "/api/sepolia-certificate")]
fn http_sepolia_certificate(ctx: &_, _req: http::Request) -> LyquidResult<http::Response> {
    let key = ctx.network.latest_sepolia_query_hash.to_string();
    let cert = ctx
        .network
        .sepolia_certificates
        .get(&key)
        .ok_or_else(|| invalid("sepolia certificate not found"))?;
    let body = serde_json::to_vec(cert).map_err(|e| invalid(e.to_string()))?;
    Ok(json_response(body))
}