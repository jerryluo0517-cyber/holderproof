//! Validate the balance fact, checkpoint, and signature evidence.

use crate::invalid;
use holderproof_types::{
    CHAIN_ID, COMMITTEE_SIZE, COMMITTEE_THRESHOLD, CertificateStatus, FIXTURE_TOKEN,
    PARSER_VERSION, QueryCertificate, QuerySpec,
};
use lyquid::prelude::*;
use lyquor_primitives::oracle::OracleCert;

pub fn validate_certificate(
    certificate: &QueryCertificate,
    holder: Address,
    block: u64,
) -> LyquidResult<()> {
    validated_runtime_certificate(certificate, holder, block).map(|_| ())
}

pub(crate) fn validated_runtime_certificate(
    certificate: &QueryCertificate,
    holder: Address,
    block: u64,
) -> LyquidResult<OracleCert> {
    ensure_finalized(certificate)?;
    ensure_sufficient_signers(certificate)?;
    ensure_query_matches(certificate, holder, block)?;
    ensure_metadata(certificate)?;
    ensure_checkpoint(certificate, block)?;
    validate_runtime_evidence(certificate)
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
    if !(COMMITTEE_THRESHOLD..=COMMITTEE_SIZE).contains(&count) {
        return Err(invalid(format!(
            "certificate has {count} signer(s), expected {COMMITTEE_THRESHOLD}–{COMMITTEE_SIZE}"
        )));
    }
    for (index, signer) in certificate.signer_ids.iter().enumerate() {
        if signer.trim().is_empty() || certificate.signer_ids[..index].contains(signer) {
            return Err(invalid("signer IDs must be nonempty and distinct"));
        }
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
    let is_sepolia = certificate.checkpoint.dataset_id == holderproof_types::SEPOLIA_DATASET_ID;
    if is_sepolia {
        match certificate.token.parse::<Address>() {
            Ok(address) if address != Address::ZERO => {}
            _ => {
                return Err(invalid(
                    "sepolia certificate token must be a valid nonzero address",
                ));
            }
        }
    } else if certificate.token != FIXTURE_TOKEN {
        return Err(invalid("certificate token does not match the fixture"));
    }
    if certificate.checkpoint.parser_version != PARSER_VERSION {
        return Err(invalid("unsupported certificate parser version"));
    }
    let query = QuerySpec {
        chain_id: CHAIN_ID,
        token: certificate.token.to_ascii_lowercase(),
        holder: certificate.holder.to_ascii_lowercase(),
        block: certificate.block_number,
        parser_version: PARSER_VERSION.to_string(),
    };
    let expected_hash = query.hash();
    if certificate.query_hash != expected_hash || certificate.certificate_id != expected_hash {
        return Err(invalid(
            "certificate query hash or ID does not match query fields",
        ));
    }
    if certificate.created_at == 0 {
        return Err(invalid("created_at is 0"));
    }
    Ok(())
}

fn ensure_checkpoint(certificate: &QueryCertificate, block: u64) -> LyquidResult<()> {
    let checkpoint = &certificate.checkpoint;
    if checkpoint.dataset_id == holderproof_types::SEPOLIA_DATASET_ID {
        if checkpoint.from_block > checkpoint.to_block || checkpoint.to_block != block {
            return Err(invalid("sepolia checkpoint block range mismatch"));
        }
        if !checkpoint
            .block_hash_start
            .strip_prefix("0x")
            .is_some_and(is_digest)
            || !checkpoint
                .block_hash
                .strip_prefix("0x")
                .is_some_and(is_digest)
            || checkpoint.event_count == 0
            || !is_digest(&checkpoint.event_merkle_root)
        {
            return Err(invalid("invalid sepolia checkpoint hashes or root"));
        }
        return Ok(());
    }
    if checkpoint.dataset_id != holderproof_types::DATASET_ID {
        return Err(invalid("unexpected fixture dataset"));
    }
    if checkpoint.block_hash_start != holderproof_types::fixture_block_hash(0)
        || checkpoint.block_hash != holderproof_types::fixture_block_hash(block)
    {
        return Err(invalid(
            "checkpoint block hashes do not match fixture source",
        ));
    }
    if checkpoint.from_block != 0 {
        return Err(invalid(format!(
            "checkpoint must replay from genesis, got from_block {}",
            checkpoint.from_block
        )));
    }
    if block > holderproof_types::FIXTURE_END_BLOCK {
        return Err(invalid("block exceeds finalized fixture range"));
    }
    if checkpoint.to_block != block {
        return Err(invalid(format!(
            "checkpoint to_block {} != requested block {block}",
            checkpoint.to_block
        )));
    }
    if checkpoint.event_count == 0 {
        return Err(invalid("checkpoint event_count is 0"));
    }
    if !is_digest(&checkpoint.event_merkle_root) {
        return Err(invalid(
            "checkpoint event_merkle_root must be a SHA-256 digest",
        ));
    }
    Ok(())
}

/// Wire hashes use exactly 32 bytes encoded as lowercase hexadecimal.
fn is_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

// Lyquor authenticates the certified write. The gate compares the exported
// fact and signers with the runtime evidence.
fn validate_runtime_evidence(certificate: &QueryCertificate) -> LyquidResult<OracleCert> {
    if certificate.query_type != holderproof_types::QueryType::Balance
        || certificate.fact_hash != certificate.fact().hash()
    {
        return Err(invalid(
            "certificate fact digest does not match its contents",
        ));
    }
    if certificate.network_state_id.is_empty() {
        return Err(invalid("network state provenance is missing"));
    }
    let native: OracleCert = serde_json::from_str(&certificate.runtime_certificate)
        .map_err(|_| invalid("invalid native runtime certificate"))?;
    let count = certificate.signer_ids.len();
    if native.signers.len() != count
        || native.signatures.len() != count
        || certificate.node_signatures.len() != count
    {
        return Err(invalid("runtime signature count mismatch"));
    }
    for (index, exported_signature) in certificate.node_signatures.iter().enumerate() {
        let runtime_signer = native.signers[index];
        let runtime_signature = &native.signatures[index];
        let signer_matches = exported_signature.node_id == certificate.signer_ids[index]
            && exported_signature.signer_id == runtime_signer;
        let signature_matches = runtime_signature.len() == 64
            && exported_signature.signature == hex::encode(runtime_signature);
        let first_occurrence = !native.signers[..index].contains(&runtime_signer);
        if !(signer_matches && signature_matches && first_occurrence) {
            return Err(invalid("inconsistent runtime signature evidence"));
        }
    }
    Ok(native)
}
