//! HolderProof Consumer Lyquid.
//!
//! Reading order:
//! 1. `state!` holds the configured query and last validated result.
//! 2. `configure` binds the Indexer and eligibility threshold.
//! 3. `validate_certificate` enforces the certificate trust rules.
//! 4. `parse_balance` converts the wire value to `U256`.
//! 5. `check_eligible` calls the Indexer and stores the result.

use holderproof_types::{CHAIN_ID, COMMITTEE_THRESHOLD, CertificateStatus, QueryCertificate};
use lyquid::prelude::*;

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
        return Err(invalid(
            "HolderProof consumer indexer is not configured",
        ));
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
    let balance = parse_balance(&certificate)?;
    let eligible = balance >= threshold;
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

/// Validate a certificate against the consumer's trust rules.
///
/// Split into one check per concern so a failure names the exact field and
/// value, instead of collapsing everything into one opaque message.
fn validate_certificate(
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

fn invalid(reason: impl Into<String>) -> LyquidError {
    LyquidError::LyquorRuntime(reason.into())
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

fn parse_balance(certificate: &QueryCertificate) -> LyquidResult<U256> {
    certificate
        .balance
        .parse::<U256>()
        .map_err(|_| invalid("invalid certificate balance"))
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
        "certificateStatus": ctx.network.last_certificate_status.to_string(),
        "signerCount": *ctx.network.last_signer_count,
        "queryHash": ctx.network.last_query_hash.to_string(),
        "blockHash": ctx.network.last_block_hash.to_string(),
        "eventMerkleRoot": ctx.network.last_event_merkle_root.to_string(),
    }))
    .map_err(|err| invalid(format!("result JSON error: {err}")))?;
    Ok(http::Response {
        status: 200,
        headers: vec![http::Header {
            name: "content-type".into(),
            value: b"application/json; charset=utf-8".to_vec(),
        }],
        body,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use holderproof_types::Checkpoint;
    use lyquid_test::test;

    fn holder() -> Address {
        Address::new([0xaa; 20])
    }

    fn valid_certificate() -> QueryCertificate {
        QueryCertificate {
            certificate_id: "cert-1".into(),
            query_hash: "query-1".into(),
            chain_id: CHAIN_ID,
            token: "0x5298000000000000000000000000000000000001".into(),
            holder: holder().to_string().to_ascii_lowercase(),
            block_number: 1000,
            balance: "62685".into(),
            checkpoint: Checkpoint {
                from_block: 0,
                to_block: 1000,
                block_hash: "0xblock".into(),
                event_count: 308,
                event_merkle_root: "0xmerkle".into(),
                parser_version: "test".into(),
            },
            signer_ids: vec!["node-1".into(), "node-2".into(), "node-3".into()],
            status: CertificateStatus::Finalized,
            created_at: 10,
        }
    }

    #[test]
    fn valid_certificate_is_accepted() {
        assert!(validate_certificate(&valid_certificate(), holder(), 1000).is_ok());
        assert_eq!(
            parse_balance(&valid_certificate()).unwrap(),
            U256::from(62_685u64)
        );
    }

    #[test]
    fn non_finalized_certificate_is_rejected() {
        let mut certificate = valid_certificate();
        certificate.status = CertificateStatus::Pending;
        assert!(validate_certificate(&certificate, holder(), 1000).is_err());
    }

    #[test]
    fn insufficient_signers_are_rejected() {
        let mut certificate = valid_certificate();
        certificate.signer_ids.pop();
        assert!(validate_certificate(&certificate, holder(), 1000).is_err());
    }

    #[test]
    fn wrong_block_is_rejected() {
        assert!(validate_certificate(&valid_certificate(), holder(), 999).is_err());
    }
}
