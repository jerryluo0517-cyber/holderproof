use lyquid::prelude::*;

state! {
    network indexer: Address = Address::ZERO;
    network expected_holder: Address = Address::ZERO;
    network expected_block: u64 = 0;
    network threshold: U256 = U256::ZERO;
    network last_balance: U256 = U256::ZERO;
    network last_eligible: bool = false;
    network check_count: u64 = 0;
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
        (indexer).get_certified_balance(
            holder: Address = holder,
            block: u64 = block
        ) -> (balance: LyquidResult<U256>)
    );
    let balance = response.balance?;
    let eligible = balance >= threshold;
    *ctx.network.last_balance = balance;
    *ctx.network.last_eligible = eligible;
    *ctx.network.check_count += 1;
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
