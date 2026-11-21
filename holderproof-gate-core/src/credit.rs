//! Trust rules and dynamic fee / LTV tiering for VeriCredit certificates.

use crate::invalid;
use holderproof_types::{
    CHAIN_ID, COMMITTEE_SIZE, COMMITTEE_THRESHOLD, CREDIT_DATASET_ID, CREDIT_PROTOCOL,
    CertificateStatus, CreditCertificate, FIXTURE_END_BLOCK, PARSER_VERSION, QueryType,
    fixture_block_hash,
};
use lyquid::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CreditTier {
    pub label: &'static str,
    pub fee_bps: u64,
    pub ltv_bps: u64,
}

pub fn evaluate_tier(score: u64) -> CreditTier {
    if score >= 800 {
        CreditTier {
            label: "VIP",
            fee_bps: 5,
            ltv_bps: 8500,
        }
    } else if score >= 500 {
        CreditTier {
            label: "Standard",
            fee_bps: 15,
            ltv_bps: 7500,
        }
    } else {
        CreditTier {
            label: "Base",
            fee_bps: 30,
            ltv_bps: 6000,
        }
    }
}

pub fn certified_credit(
    indexer: Address,
    user: Address,
    start_block: u64,
    end_block: u64,
) -> LyquidResult<CreditCertificate> {
    let response = call!(
        (LyquidID::from(indexer)).get_credit_certificate_json(
            user: Address = user,
            start_block: u64 = start_block,
            end_block: u64 = end_block
        ) -> (certificate: LyquidResult<String>)
    );
    let certificate: CreditCertificate = serde_json::from_str(&response.certificate?)
        .map_err(|err| invalid(format!("credit certificate decode error: {err}")))?;
    validate_credit_certificate(&certificate, user, start_block, end_block)?;
    let native: lyquor_primitives::oracle::OracleCert =
        serde_json::from_str(&certificate.runtime_certificate)
            .map_err(|_| invalid("invalid native credit certificate"))?;
    let source = LyquidID::from(indexer);
    if native.header.target.target != OracleServiceTarget::LVM(source)
        || native.header.target.seq_id != lyquor_api::sequence_backend_id()?
        || certificate.network_state_id != format!("{}@{}", source, certificate.created_at)
    {
        return Err(invalid(
            "credit certificate provenance does not match configured Indexer",
        ));
    }
    Ok(certificate)
}

pub fn validate_credit_certificate(
    certificate: &CreditCertificate,
    user: Address,
    start_block: u64,
    end_block: u64,
) -> LyquidResult<()> {
    if certificate.status != CertificateStatus::Finalized {
        return Err(invalid("credit certificate is not Finalized"));
    }
    let count = certificate.signer_ids.len();
    if !(COMMITTEE_THRESHOLD..=COMMITTEE_SIZE).contains(&count) {
        return Err(invalid("credit certificate signer count out of range"));
    }
    for (i, signer) in certificate.signer_ids.iter().enumerate() {
        if signer.trim().is_empty() || certificate.signer_ids[..i].contains(signer) {
            return Err(invalid("credit signer IDs must be nonempty and distinct"));
        }
    }
    let user_lower = user.to_string().to_ascii_lowercase();
    if certificate.query.user.to_ascii_lowercase() != user_lower
        || certificate.query.start_block != start_block
        || certificate.query.end_block != end_block
    {
        return Err(invalid("credit certificate query does not match request"));
    }
    if certificate.query.chain_id != CHAIN_ID
        || certificate.query.protocol != CREDIT_PROTOCOL
        || certificate.query.parser_version != PARSER_VERSION
        || certificate.created_at == 0
    {
        return Err(invalid("invalid credit certificate metadata"));
    }
    let expected_hash = certificate.query.hash();
    if certificate.query_hash != expected_hash || certificate.certificate_id != expected_hash {
        return Err(invalid("credit query hash or ID mismatch"));
    }
    let cp = &certificate.checkpoint;
    if cp.dataset_id != CREDIT_DATASET_ID
        || cp.from_block != start_block
        || cp.to_block != end_block
        || end_block > FIXTURE_END_BLOCK
        || cp.block_hash_start != fixture_block_hash(start_block)
        || cp.block_hash != fixture_block_hash(end_block)
        || cp.event_count == 0
        || !is_digest(&cp.event_merkle_root)
    {
        return Err(invalid("invalid credit checkpoint"));
    }
    if certificate.query_type != QueryType::CreditScore
        || certificate.fact_hash != certificate.fact().hash()
        || certificate.network_state_id.is_empty()
        || certificate.metrics.score > 1000
    {
        return Err(invalid("invalid credit fact or score"));
    }
    let native: lyquor_primitives::oracle::OracleCert =
        serde_json::from_str(&certificate.runtime_certificate)
            .map_err(|_| invalid("invalid native runtime certificate"))?;
    if native.signers.len() != count
        || native.signatures.len() != count
        || certificate.node_signatures.len() != count
    {
        return Err(invalid("credit signature count mismatch"));
    }
    for (i, sig) in certificate.node_signatures.iter().enumerate() {
        if sig.node_id != certificate.signer_ids[i]
            || sig.signer_id != native.signers[i]
            || sig.signature != hex::encode(&native.signatures[i])
            || native.signatures[i].len() != 64
            || native.signers[..i].contains(&sig.signer_id)
        {
            return Err(invalid("inconsistent credit runtime signature evidence"));
        }
    }
    Ok(())
}

fn is_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}