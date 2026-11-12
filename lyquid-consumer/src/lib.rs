//! HolderProof Consumer Lyquid.
//!
//! Reading order:
//! 1. `state!` holds the configured query and latest validated result.
//! 2. `configure` binds the Indexer, holder, block and threshold.
//! 3. `check_eligible` fetches the certificate from the Indexer.
//! 4. `certificate.rs` validates the trust rules.
//! 5. `eligibility.rs` parses the balance and computes eligibility.
//! 6. `http_endpoints.rs` exposes the diagnostic result.

use certificate::validate_certificate;
use eligibility::evaluate;
use holderproof_types::QueryCertificate;
use lyquid::prelude::*;

mod certificate;
mod eligibility;
mod http_endpoints;
#[cfg(test)]
mod tests;

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
        return Err(invalid("HolderProof consumer indexer is not configured"));
    }

    let indexer = LyquidID::from(indexer_address);
    let response = call!(
        (indexer).get_certificate_json(
            holder: Address = holder,
            block: u64 = block
        ) -> (certificate: LyquidResult<String>)
    );
    let certificate: QueryCertificate = serde_json::from_str(&response.certificate?)
        .map_err(|err| invalid(format!("certificate decode error: {err}")))?;

    validate_certificate(&certificate, holder, block)?;
    let (balance, eligible) = evaluate(&certificate, threshold)?;

    *ctx.network.last_balance = balance;
    *ctx.network.last_eligible = eligible;
    *ctx.network.check_count += 1;
    *ctx.network.last_query_hash = certificate.query_hash;
    *ctx.network.last_block_hash = certificate.checkpoint.block_hash;
    *ctx.network.last_event_merkle_root = certificate.checkpoint.event_merkle_root;
    *ctx.network.last_certificate_status = certificate.status.to_string();
    *ctx.network.last_signer_count = certificate.signer_ids.len() as u64;
    Ok(eligible)
}

/// Build a runtime error without repeating `LyquidError::LyquorRuntime(...into())`.
pub(crate) fn invalid(reason: impl Into<String>) -> LyquidError {
    LyquidError::LyquorRuntime(reason.into())
}
