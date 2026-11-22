//! Shared certificate and policy rules for the Airdrop, Governance, and Credit gates.

use holderproof_types::QueryCertificate;
use lyquid::prelude::*;
pub mod certificate;
pub mod configuration;
pub mod credit;
pub mod eligibility;
#[cfg(test)]
mod tests;

pub fn invalid(reason: impl Into<String>) -> LyquidError {
    LyquidError::LyquorRuntime(reason.into())
}

/// Fetch and validate the Indexer's balance certificate from a network method.
pub fn certified_balance(
    indexer: Address,
    holder: Address,
    block: u64,
) -> LyquidResult<QueryCertificate> {
    let source = LyquidID::from(indexer);
    let response = call!(
        (source).get_certificate_json(holder: Address = holder, block: u64 = block)
        -> (certificate: LyquidResult<String>)
    );
    let certificate: QueryCertificate = serde_json::from_str(&response.certificate?)
        .map_err(|err| invalid(format!("certificate decode error: {err}")))?;
    let native = certificate::validated_runtime_certificate(&certificate, holder, block)?;
    let expected_state = format!("{source}@{}", certificate.created_at);
    if native.header.target.target != OracleServiceTarget::LVM(source)
        || native.header.target.seq_id != lyquor_api::sequence_backend_id()?
        || certificate.network_state_id != expected_state
    {
        return Err(invalid(
            "certificate provenance does not match configured Indexer",
        ));
    }
    Ok(certificate)
}
