//! Assemble wire evidence after the runtime has authenticated a certified call.
//!
//! Runtime signer resolution and network-state writes stay in lib.rs; this
//! module only validates the signer list and constructs the certificate value.

use crate::invalid;
use holderproof_types::{
    BalanceFact, COMMITTEE_THRESHOLD, CertificateStatus, NodeSignature, QueryCertificate,
};
use lyquid::prelude::*;
use lyquor_primitives::oracle::OracleCert;

pub(crate) fn build(
    fact: &BalanceFact,
    node_signatures: Vec<NodeSignature>,
    runtime_certificate: &OracleCert,
    indexer: LyquidID,
    created_at: u64,
) -> LyquidResult<QueryCertificate> {
    let signer_ids: Vec<String> = node_signatures
        .iter()
        .map(|signature| signature.node_id.clone())
        .collect();
    ensure_distinct_quorum(&signer_ids)?;
    let query_hash = fact.query.hash();
    let runtime_certificate = serde_json::to_string(runtime_certificate)
        .map_err(|error| invalid(format!("runtime certificate JSON error: {error}")))?;

    Ok(QueryCertificate {
        query_type: fact.query_type,
        fact_hash: fact.hash(),
        node_signatures,
        runtime_certificate,
        network_state_id: format!("{indexer}@{created_at}"),
        certificate_id: query_hash.clone(),
        query_hash,
        chain_id: fact.query.chain_id,
        token: fact.query.token.clone(),
        holder: fact.query.holder.clone(),
        block_number: fact.query.block,
        balance: fact.balance.clone(),
        checkpoint: fact.checkpoint.clone(),
        signer_ids,
        status: CertificateStatus::Finalized,
        created_at,
    })
}

fn ensure_distinct_quorum(signers: &[String]) -> LyquidResult<()> {
    let has_duplicate = signers
        .iter()
        .enumerate()
        .any(|(index, signer)| signers[..index].contains(signer));
    if signers.len() < COMMITTEE_THRESHOLD || has_duplicate {
        return Err(invalid(
            "certificate requires two distinct committee signers",
        ));
    }
    Ok(())
}
