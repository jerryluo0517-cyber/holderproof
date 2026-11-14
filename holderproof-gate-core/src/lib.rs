//! Shared trust and threshold rules for independent Airdrop and Governance Lyquids.

use holderproof_types::QueryCertificate;
use lyquid::prelude::*;
pub mod certificate;
pub mod configuration;
pub mod eligibility;
#[cfg(test)]
mod tests;

pub fn invalid(reason: impl Into<String>) -> LyquidError {
    LyquidError::LyquorRuntime(reason.into())
}

/// Network-only: call! has atomic inter-Lyquid semantics in LDK 0.8.4.
/// Keep canonical policy in network state so every node evaluates the same rule.
pub fn certified_balance(
    indexer: Address,
    holder: Address,
    block: u64,
) -> LyquidResult<QueryCertificate> {
    let response = call!(
        (LyquidID::from(indexer)).get_certificate_json(holder: Address = holder, block: u64 = block)
        -> (certificate: LyquidResult<String>)
    );
    let certificate: QueryCertificate = serde_json::from_str(&response.certificate?)
        .map_err(|err| invalid(format!("certificate decode error: {err}")))?;
    certificate::validate_certificate(&certificate, holder, block)?;
    let native: lyquor_primitives::oracle::OracleCert =
        serde_json::from_str(&certificate.runtime_certificate)
            .map_err(|_| invalid("invalid native certificate"))?;
    let source = LyquidID::from(indexer);
    if native.header.target.target != OracleServiceTarget::LVM(source)
        || native.header.target.seq_id != lyquor_api::sequence_backend_id()?
        || certificate.network_state_id != format!("{}@{}", source, certificate.created_at)
    {
        return Err(invalid(
            "certificate provenance does not match configured Indexer",
        ));
    }
    Ok(certificate)
}
