//! HTTP diagnostics endpoints for the Indexer Lyquid.
//!
//! These make the cross-Lyquid flow observable without relying on an Ethereum
//! static call. They are registered via `#[method::instance(export = http)]`.
//!
//! `use super::*` brings in the `state!`-generated context type so the
//! handlers can read `ctx.network.*`, plus the `invalid` error helper.

use super::*;

#[method::instance(export = http, method = "GET", path_prefix = "/api/balance")]
fn http_balance(ctx: &_, _req: http::Request) -> LyquidResult<http::Response> {
    let body = serde_json::to_vec(&serde_json::json!({
        "holder": ctx.network.snapshot_holder.to_string(),
        "block": *ctx.network.snapshot_block,
        "balance": ctx.network.snapshot_balance.to_string(),
    }))
    .map_err(|err| invalid(format!("balance JSON error: {err}")))?;

    Ok(http::Response {
        status: 200,
        headers: vec![http::Header {
            name: "content-type".into(),
            value: b"application/json; charset=utf-8".to_vec(),
        }],
        body,
    })
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
        .ok_or_else(|| invalid("certificate not found"))?;
    let body = serde_json::to_vec(certificate)
        .map_err(|err| invalid(format!("certificate JSON error: {err}")))?;

    Ok(http::Response {
        status: 200,
        headers: vec![http::Header {
            name: "content-type".into(),
            value: b"application/json; charset=utf-8".to_vec(),
        }],
        body,
    })
}
