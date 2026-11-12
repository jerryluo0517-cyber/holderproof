//! Certificate trust rules for the HolderProof Consumer.
//!
//! Each rule is a small function so a rejected certificate reports the exact
//! field that failed.

use crate::invalid;
use holderproof_types::{CHAIN_ID, COMMITTEE_THRESHOLD, CertificateStatus, QueryCertificate};
use lyquid::prelude::*;

pub(crate) fn validate_certificate(
    certificate: &QueryCertificate,
    holder: Address,
    block: u64,
) -> LyquidResult<()> {
    ensure_finalized(certificate)?;
    ensure_sufficient_signers(certificate)?;
    ensure_query_matches(certificate, holder, block)?;
    ensure_metadata(certificate)?;
    ensure_checkpoint(certificate, block)?;
    Ok(())
}

fn ensure_finalized(certificate: &QueryCertificate) -> LyquidResult<()> {
    if certificate.status != CertificateStatus::Finalized {
        return Err(invalid(format!(
            "certificate status is {:?}, expected Finalized",
            certificate.status
        )));
    }
    Ok(())
}

fn ensure_sufficient_signers(certificate: &QueryCertificate) -> LyquidResult<()> {
    let count = certificate.signer_ids.len();
    if count < COMMITTEE_THRESHOLD {
        return Err(invalid(format!(
            "certificate has {count} signer(s), need at least {COMMITTEE_THRESHOLD}"
        )));
    }
    Ok(())
}

fn ensure_query_matches(
    certificate: &QueryCertificate,
    holder: Address,
    block: u64,
) -> LyquidResult<()> {
    let holder_lower = holder.to_string().to_ascii_lowercase();
    if certificate.holder.to_ascii_lowercase() != holder_lower {
        return Err(invalid(format!(
            "certificate holder {} does not match requested {}",
            certificate.holder, holder_lower
        )));
    }
    if certificate.block_number != block {
        return Err(invalid(format!(
            "certificate block {} does not match requested {block}",
            certificate.block_number
        )));
    }
    Ok(())
}

fn ensure_metadata(certificate: &QueryCertificate) -> LyquidResult<()> {
    if certificate.chain_id != CHAIN_ID {
        return Err(invalid(format!(
            "certificate chain_id {} != expected {CHAIN_ID}",
            certificate.chain_id
        )));
    }
    if certificate.certificate_id.is_empty() {
        return Err(invalid("certificate_id is empty"));
    }
    if certificate.query_hash.is_empty() {
        return Err(invalid("query_hash is empty"));
    }
    if certificate.token.is_empty() {
        return Err(invalid("token is empty"));
    }
    if certificate.created_at == 0 {
        return Err(invalid("created_at is 0"));
    }
    Ok(())
}

fn ensure_checkpoint(certificate: &QueryCertificate, block: u64) -> LyquidResult<()> {
    let checkpoint = &certificate.checkpoint;
    if checkpoint.from_block > checkpoint.to_block {
        return Err(invalid(format!(
            "checkpoint from_block {} > to_block {}",
            checkpoint.from_block, checkpoint.to_block
        )));
    }
    if checkpoint.to_block != block {
        return Err(invalid(format!(
            "checkpoint to_block {} != requested block {block}",
            checkpoint.to_block
        )));
    }
    if checkpoint.block_hash.is_empty() {
        return Err(invalid("checkpoint block_hash is empty"));
    }
    if checkpoint.event_count == 0 {
        return Err(invalid("checkpoint event_count is 0"));
    }
    if checkpoint.event_merkle_root.is_empty() {
        return Err(invalid("checkpoint event_merkle_root is empty"));
    }
    if checkpoint.parser_version.is_empty() {
        return Err(invalid("checkpoint parser_version is empty"));
    }
    Ok(())
}
