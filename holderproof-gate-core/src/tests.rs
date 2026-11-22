use crate::certificate::validate_certificate;
use crate::eligibility::evaluate;
use holderproof_types::{
    CHAIN_ID, CertificateStatus, Checkpoint, FIXTURE_TOKEN, PARSER_VERSION, QueryCertificate,
    QuerySpec,
};
use lyquid::prelude::*;
use lyquid_test::test;

fn holder() -> Address {
    Address::new([0xaa; 20])
}

fn valid_certificate() -> QueryCertificate {
    let query = QuerySpec {
        chain_id: CHAIN_ID,
        token: FIXTURE_TOKEN.into(),
        holder: holder().to_string().to_ascii_lowercase(),
        block: 1000,
        parser_version: PARSER_VERSION.into(),
    };
    let mut certificate = QueryCertificate {
        query_type: holderproof_types::QueryType::Balance,
        fact_hash: String::new(),
        node_signatures: Vec::new(),
        runtime_certificate: String::new(),
        network_state_id: "test-indexer@10".into(),
        certificate_id: query.hash(),
        query_hash: query.hash(),
        chain_id: CHAIN_ID,
        token: "0x5298000000000000000000000000000000000001".into(),
        holder: holder().to_string().to_ascii_lowercase(),
        block_number: 1000,
        balance: "62685".into(),
        checkpoint: Checkpoint {
            dataset_id: "holderproof-fixture-5298-v1".into(),
            block_hash_start: holderproof_types::fixture_block_hash(0),
            from_block: 0,
            to_block: 1000,
            block_hash: holderproof_types::fixture_block_hash(1000),
            event_count: 308,
            event_merkle_root: "cd".repeat(32),
            parser_version: PARSER_VERSION.into(),
        },
        signer_ids: vec!["node-1".into(), "node-2".into(), "node-3".into()],
        status: CertificateStatus::Finalized,
        created_at: 10,
    };
    certificate.fact_hash = certificate.fact().hash();
    set_signatures(&mut certificate);
    certificate
}

// Synthetic native evidence tests structure only. Cryptographic authenticity
// is verified by the real LDK certified-write path in the end-to-end demo.
fn set_signatures(certificate: &mut QueryCertificate) {
    use lyquor_primitives::oracle::{OracleCert, OracleHeader};
    let native = OracleCert {
        header: OracleHeader {
            proposer: NodeID::new([0; 32]),
            target: OracleTarget {
                target: OracleServiceTarget::LVM(LyquidID::default()),
                seq_id: [0u8; 32].into(),
            },
            config_hash: [0u8; 32].into(),
            epoch: 1,
            nonce: [0u8; 32].into(),
        },
        signers: (0..certificate.signer_ids.len() as u32).collect(),
        signatures: certificate
            .signer_ids
            .iter()
            .map(|_| vec![1u8; 64].into())
            .collect(),
    };
    certificate.node_signatures = certificate
        .signer_ids
        .iter()
        .enumerate()
        .map(|(index, node)| holderproof_types::NodeSignature {
            node_id: node.clone(),
            signer_id: index as u32,
            signature: hex::encode(&native.signatures[index]),
        })
        .collect();
    certificate.runtime_certificate = serde_json::to_string(&native).unwrap();
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
    for status in [
        CertificateStatus::Pending,
        CertificateStatus::Challenged,
        CertificateStatus::Invalidated,
    ] {
        let mut certificate = valid_certificate();
        certificate.status = status;
        assert!(validate_certificate(&certificate, holder(), 1000).is_err());
    }
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

#[test]
fn duplicate_or_empty_signers_are_rejected() {
    let mut certificate = valid_certificate();
    certificate.signer_ids[1] = certificate.signer_ids[0].clone();
    assert!(validate_certificate(&certificate, holder(), 1000).is_err());
    certificate.signer_ids[1] = " ".into();
    assert!(validate_certificate(&certificate, holder(), 1000).is_err());
}

#[test]
fn tampered_identity_fields_are_rejected() {
    for field in [
        "chain",
        "token",
        "holder",
        "parser",
        "query_hash",
        "certificate_id",
    ] {
        let mut certificate = valid_certificate();
        match field {
            "chain" => certificate.chain_id += 1,
            "token" => certificate.token = Address::ZERO.to_string(),
            "holder" => certificate.holder = Address::ZERO.to_string(),
            "parser" => certificate.checkpoint.parser_version = "unknown".into(),
            "query_hash" => certificate.query_hash = "00".repeat(32),
            "certificate_id" => certificate.certificate_id = "00".repeat(32),
            _ => unreachable!(),
        }
        assert!(
            validate_certificate(&certificate, holder(), 1000).is_err(),
            "{field}"
        );
    }
}

#[test]
fn invalid_checkpoints_are_rejected() {
    for field in ["from", "to", "block_hash", "root", "events", "created_at"] {
        let mut certificate = valid_certificate();
        match field {
            "from" => certificate.checkpoint.from_block = 1,
            "to" => certificate.checkpoint.to_block = 999,
            "block_hash" => certificate.checkpoint.block_hash = "0xblock".into(),
            "root" => certificate.checkpoint.event_merkle_root = "zz".repeat(32),
            "events" => certificate.checkpoint.event_count = 0,
            "created_at" => certificate.created_at = 0,
            _ => unreachable!(),
        }
        assert!(
            validate_certificate(&certificate, holder(), 1000).is_err(),
            "{field}"
        );
    }
}

#[test]
fn eligibility_handles_boundary_and_invalid_balance() {
    let mut certificate = valid_certificate();
    assert!(evaluate(&certificate, U256::from(62_685u64)).unwrap().1);
    assert!(!evaluate(&certificate, U256::from(62_686u64)).unwrap().1);
    for balance in [
        "-1",
        "invalid",
        "115792089237316195423570985008687907853269984665640564039457584007913129639936",
    ] {
        certificate.balance = balance.into();
        assert!(evaluate(&certificate, U256::ZERO).is_err());
    }
}

#[test]
fn fourth_distinct_signer_is_accepted_but_fifth_is_rejected() {
    let mut certificate = valid_certificate();
    certificate.signer_ids.push("node-4".into());
    set_signatures(&mut certificate);
    assert!(validate_certificate(&certificate, holder(), 1000).is_ok());
    certificate.signer_ids.push("node-5".into());
    set_signatures(&mut certificate);
    assert!(validate_certificate(&certificate, holder(), 1000).is_err());
}

#[test]
fn only_initializer_can_configure_nonzero_addresses() {
    let owner = Address::new([0x11; 20]);
    let stranger = Address::new([0x22; 20]);
    let indexer = Address::new([0x33; 20]);
    assert!(crate::configuration::validate(owner, owner, indexer, holder()).is_ok());
    assert!(crate::configuration::validate(stranger, owner, indexer, holder()).is_err());
    assert!(crate::configuration::validate(owner, owner, Address::ZERO, holder()).is_err());
    assert!(crate::configuration::validate(owner, owner, indexer, Address::ZERO).is_err());
}

#[test]
fn well_formed_but_wrong_block_hash_is_rejected() {
    let mut certificate = valid_certificate();
    certificate.checkpoint.block_hash = format!("0x{}", "ab".repeat(32));
    certificate.fact_hash = certificate.fact().hash();
    assert!(validate_certificate(&certificate, holder(), 1000).is_err());
}

#[test]
fn tampered_balance_root_or_runtime_evidence_is_rejected() {
    for field in ["balance", "root", "signature", "native", "provenance"] {
        let mut certificate = valid_certificate();
        match field {
            "balance" => certificate.balance = "999999".into(),
            "root" => certificate.checkpoint.event_merkle_root = "00".repeat(32),
            "signature" => certificate.node_signatures[0].signature = "00".repeat(64),
            "native" => certificate.runtime_certificate = "{}".into(),
            "provenance" => certificate.network_state_id.clear(),
            _ => unreachable!(),
        }
        assert!(
            validate_certificate(&certificate, holder(), 1000).is_err(),
            "{field}"
        );
    }
}

#[test]
fn credit_tier_boundaries_map_fee_and_ltv() {
    use crate::credit::evaluate_tier;
    assert_eq!(evaluate_tier(800).label, "VIP");
    assert_eq!(evaluate_tier(800).fee_bps, 5);
    assert_eq!(evaluate_tier(800).ltv_bps, 8500);
    assert_eq!(evaluate_tier(799).label, "Standard");
    assert_eq!(evaluate_tier(500).fee_bps, 15);
    assert_eq!(evaluate_tier(499).label, "Base");
    assert_eq!(evaluate_tier(499).fee_bps, 30);
    assert_eq!(evaluate_tier(499).ltv_bps, 6000);
}

#[test]
fn credit_certificate_valid_and_tampered_rejection() {
    use crate::credit::validate_credit_certificate;
    use holderproof_types::{
        CREDIT_DATASET_ID, CREDIT_PROTOCOL, CreditCertificate, CreditMetrics, CreditParams,
        CreditQuerySpec, NodeSignature, QueryType, fixture_block_hash,
    };
    use lyquor_primitives::oracle::{OracleCert, OracleHeader};

    let user = holder();
    let query = CreditQuerySpec {
        chain_id: CHAIN_ID,
        protocol: CREDIT_PROTOCOL.into(),
        user: user.to_string().to_ascii_lowercase(),
        start_block: 0,
        end_block: 1000,
        params: CreditParams::default(),
        parser_version: PARSER_VERSION.into(),
    };
    let native = OracleCert {
        header: OracleHeader {
            proposer: NodeID::new([0; 32]),
            target: OracleTarget {
                target: OracleServiceTarget::LVM(LyquidID::default()),
                seq_id: [0u8; 32].into(),
            },
            config_hash: [0u8; 32].into(),
            epoch: 1,
            nonce: [0u8; 32].into(),
        },
        signers: vec![0, 1, 2],
        signatures: vec![vec![1u8; 64].into(), vec![2u8; 64].into(), vec![3u8; 64].into()],
    };
    let mut cert = CreditCertificate {
        query_type: QueryType::CreditScore,
        fact_hash: String::new(),
        node_signatures: vec![
            NodeSignature { node_id: "node-1".into(), signer_id: 0, signature: hex::encode(&native.signatures[0]) },
            NodeSignature { node_id: "node-2".into(), signer_id: 1, signature: hex::encode(&native.signatures[1]) },
            NodeSignature { node_id: "node-3".into(), signer_id: 2, signature: hex::encode(&native.signatures[2]) },
        ],
        runtime_certificate: serde_json::to_string(&native).unwrap(),
        network_state_id: "test-indexer@10".into(),
        certificate_id: query.hash(),
        query_hash: query.hash(),
        query,
        metrics: CreditMetrics {
            raw_volume: "1200000".into(),
            clean_volume: "1200000".into(),
            wash_volume_removed: "0".into(),
            repay_count: 12,
            liquidation_count: 0,
            score: 940,
        },
        checkpoint: Checkpoint {
            dataset_id: CREDIT_DATASET_ID.into(),
            block_hash_start: fixture_block_hash(0),
            from_block: 0,
            to_block: 1000,
            block_hash: fixture_block_hash(1000),
            event_count: 35,
            event_merkle_root: "ab".repeat(32),
            parser_version: PARSER_VERSION.into(),
        },
        signer_ids: vec!["node-1".into(), "node-2".into(), "node-3".into()],
        status: CertificateStatus::Finalized,
        created_at: 10,
    };
    cert.fact_hash = cert.fact().hash();

    assert!(validate_credit_certificate(&cert, user, 0, 1000).is_ok());

    let mut bad_score = cert.clone();
    bad_score.metrics.score = 1000;
    assert!(validate_credit_certificate(&bad_score, user, 0, 1000).is_err());

    let mut revoked = cert.clone();
    revoked.status = CertificateStatus::Invalidated;
    assert!(validate_credit_certificate(&revoked, user, 0, 1000).is_err());

    assert!(validate_credit_certificate(&cert, user, 0, 999).is_err());
}

#[test]
fn credit_report_staleness_ttl_is_enforced() {
    use crate::credit::is_stale;
    // created_at = 50, max_staleness = 200 -> 250 块以内有效，251 块起过期回退
    assert!(!is_stale(50, 50, 200));
    assert!(!is_stale(250, 50, 200));
    assert!(is_stale(251, 50, 200));
    // 防 u64::MAX 加法溢出
    assert!(!is_stale(u64::MAX, u64::MAX, 200));
}