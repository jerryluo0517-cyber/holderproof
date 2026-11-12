use crate::certificate::validate_certificate;
use crate::eligibility::evaluate;
use holderproof_types::{CHAIN_ID, CertificateStatus, Checkpoint, QueryCertificate};
use lyquid::prelude::*;
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
    let (balance, eligible) = evaluate(&valid_certificate(), U256::from(60_000u64)).unwrap();
    assert_eq!(balance, U256::from(62_685u64));
    assert!(eligible);
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
