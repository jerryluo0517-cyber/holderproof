use crate::fixture::{fixture_balance, fixture_checkpoint, fixture_events, fixture_twab};
use lyquid::prelude::*;
use lyquid_test::test;

#[test]
fn fixture_has_expected_event_count() {
    assert_eq!(fixture_events().unwrap().len(), 308);
}

#[test]
fn alice_snapshots_match_python_ground_truth() {
    let alice = Address::new([0xaa; 20]);
    assert_eq!(fixture_balance(alice, 900).unwrap(), U256::from(47_685u64));
    assert_eq!(fixture_balance(alice, 950).unwrap(), U256::from(57_685u64));
    assert_eq!(fixture_balance(alice, 1000).unwrap(), U256::from(62_685u64));
}

#[test]
fn unknown_holder_is_zero() {
    let unknown = Address::new([0x99; 20]);
    assert_eq!(fixture_balance(unknown, 1000).unwrap(), U256::ZERO);
}

#[test]
fn merkle_root_matches_python_ground_truth() {
    let checkpoint = fixture_checkpoint(1000).unwrap();
    assert_eq!(checkpoint.event_count, 308);
    assert_eq!(
        checkpoint.event_merkle_root,
        "2e89babbc7b45637458bd08a2d843087354bf175eb559e5c1ddc7fcc0e79617b"
    );
}

#[test]
fn twab_matches_python_ground_truth() {
    let alice = Address::new([0xaa; 20]);
    let bob = Address::new([0xbb; 20]);
    assert_eq!(
        fixture_twab(alice, 0, 1000, 1_000_000).unwrap(),
        U256::from(48_945_825_000u64)
    );
    assert_eq!(
        fixture_twab(bob, 0, 1000, 1_000_000).unwrap(),
        U256::from(47_662_030_000u64)
    );
}

fn proposal(balance: u64) -> crate::models::BalanceProposal {
    let mut fact = crate::fixture::fixture_fact(Address::new([0xaa; 20]), 1000).unwrap();
    fact.balance = balance.to_string();
    fact
}

#[test]
fn quorum_survives_a_first_dissenting_proposal() {
    let votes = vec![(0, proposal(1)), (1, proposal(62685)), (2, proposal(62685))];
    assert_eq!(
        crate::models::agreeing_proposal(&votes, 2),
        Some(&proposal(62685))
    );
}

#[test]
fn duplicate_nodes_and_split_votes_cannot_form_quorum() {
    let duplicates = vec![(0, proposal(1)), (0, proposal(1)), (1, proposal(1))];
    assert!(crate::models::agreeing_proposal(&duplicates, 2).is_none());
    let split = vec![(0, proposal(1)), (1, proposal(2)), (2, proposal(3))];
    let threshold = holderproof_types::COMMITTEE_THRESHOLD;
    assert!(crate::models::agreeing_proposal(&split, threshold).is_none());
}

#[test]
fn proposals_must_agree_on_holder_and_block_too() {
    let mut wrong_holder = proposal(1);
    wrong_holder.query.holder = Address::new([0xbb; 20]).to_string();
    let mut wrong_block = proposal(1);
    wrong_block.query.block = 999;
    let votes = vec![(0, proposal(1)), (1, wrong_holder), (2, wrong_block)];
    assert!(crate::models::agreeing_proposal(&votes, 2).is_none());
}

#[test]
fn committee_requires_four_distinct_members() {
    assert!(crate::models::valid_committee(&[0, 1, 2, 3]));
    assert!(!crate::models::valid_committee(&[0, 1, 2]));
    assert!(!crate::models::valid_committee(&[0, 1, 2, 2]));
    assert!(!crate::models::valid_committee(&[0, 1, 2, 3, 4]));
}

#[test]
fn query_hash_remains_compatible_with_existing_certificates() {
    let query = crate::fixture::make_query_spec(
        holderproof_types::FIXTURE_TOKEN,
        Address::new([0xaa; 20]),
        1000,
    );
    assert_eq!(
        query.hash(),
        "1613062534e5d67e47f7bff3afe047f17be2e8f724311d9005eb41681279a884"
    );
}

#[test]
fn checkpoint_or_dataset_disagreement_prevents_quorum() {
    for field in ["root", "hash", "dataset", "count"] {
        let good = proposal(1);
        let mut bad = good.clone();
        match field {
            "root" => bad.checkpoint.event_merkle_root = "00".repeat(32),
            "hash" => bad.checkpoint.block_hash = "0x00".into(),
            "dataset" => bad.checkpoint.dataset_id = "other".into(),
            "count" => bad.checkpoint.event_count += 1,
            _ => unreachable!(),
        }
        let votes = vec![(0, good), (1, bad), (2, proposal(2))];
        assert!(
            crate::models::agreeing_proposal(&votes, 2).is_none(),
            "{field}"
        );
    }
}

#[test]
fn twab_respects_half_open_window_and_large_gaps() {
    let alice = Address::new([0xaa; 20]);
    let expected = (fixture_balance(alice, 949).unwrap() + fixture_balance(alice, 950).unwrap())
        / U256::from(2u64);
    assert_eq!(fixture_twab(alice, 949, 951, 1).unwrap(), expected);
    assert!(fixture_twab(alice, 950, 950, 1).is_err());
    assert_eq!(
        fixture_twab(alice, 1000, 1_000_000_000, 1).unwrap(),
        U256::from(62685u64)
    );
}

#[test]
fn same_block_replay_uses_log_order_and_includes_the_boundary() {
    use crate::fixture::{parse_events, replay_balance};

    let holder = Address::new([0xaa; 20]);
    let recipient = Address::new([0xbb; 20]);
    let mut credit = fixture_events().unwrap()[0].clone();
    credit.block_number = 10;
    credit.block_hash = holderproof_types::fixture_block_hash(10);
    credit.log_index = 0;
    credit.from = Address::ZERO.to_string();
    credit.to = holder.to_string();
    credit.amount = "10".into();

    let mut debit = credit.clone();
    debit.log_index = 1;
    debit.from = holder.to_string();
    debit.to = recipient.to_string();
    debit.amount = "4".into();

    // Reverse the input so parsing must restore log order before the debit.
    let json = serde_json::to_string(&[debit, credit]).unwrap();
    let events = parse_events(&json).unwrap();
    assert_eq!(replay_balance(&events, holder, 9).unwrap(), U256::ZERO);
    assert_eq!(replay_balance(&events, holder, 10).unwrap(), U256::from(6));
}

#[test]
fn vericredit_scores_match_python_ground_truth() {
    use crate::credit::{credit_events, evaluate_credit};
    use holderproof_types::CreditParams;

    let events = credit_events().unwrap();
    let params = CreditParams::default();

    let alice = evaluate_credit(events, Address::new([0xaa; 20]), 0, 1000, params).unwrap();
    assert_eq!(alice.clean_volume, "1200000");
    assert_eq!(alice.wash_volume_removed, "0");
    assert_eq!(alice.score, 940);

    let bob = evaluate_credit(events, Address::new([0xbb; 20]), 0, 1000, params).unwrap();
    assert_eq!(bob.clean_volume, "15000");
    assert_eq!(bob.wash_volume_removed, "2000000");
    assert_eq!(bob.score, 495);

    let carol = evaluate_credit(events, Address::new([0xcc; 20]), 0, 1000, params).unwrap();
    assert_eq!(carol.liquidation_count, 1);
    assert_eq!(carol.score, 250);

    let dave = evaluate_credit(events, Address::new([0xdd; 20]), 0, 1000, params).unwrap();
    assert_eq!(dave.score, 675);
}

#[test]
fn vericredit_custom_params_change_score_and_query_hash() {
    use crate::credit::credit_fact;
    use holderproof_types::CreditParams;

    let bob = Address::new([0xbb; 20]);
    let default_fact = credit_fact(bob, 0, 1000, CreditParams::default()).unwrap();
    // 把洗盘窗口设为 0（仅剔除同块洗盘），Bob 的跨块对敲不再被剔除，分数升高且 query_hash 不同
    let lax = CreditParams { wash_window: 0, ..CreditParams::default() };
    let lax_fact = credit_fact(bob, 0, 1000, lax).unwrap();
    assert!(lax_fact.metrics.score > default_fact.metrics.score);
    assert_ne!(lax_fact.query.hash(), default_fact.query.hash());
}

#[test]
fn vericredit_quorum_works_with_one_dissenter() {
    use crate::credit::credit_fact;
    use holderproof_types::CreditParams;

    let good = credit_fact(Address::new([0xaa; 20]), 0, 1000, CreditParams::default()).unwrap();
    let mut bad = good.clone();
    bad.metrics.score = 100;
    let votes = vec![(0, bad), (1, good.clone()), (2, good.clone())];
    assert_eq!(crate::models::agreeing_proposal(&votes, 2), Some(&good));
}

#[test]
fn periodic_epoch_checkpoints_advance_monotonically() {
    use crate::fixture::fixture_epoch_fact;

    let ep1 = fixture_epoch_fact(250, 250).unwrap();
    let ep2 = fixture_epoch_fact(500, 250).unwrap();
    let ep4 = fixture_epoch_fact(1000, 250).unwrap();
    assert_eq!(ep1.epoch_id, 1);
    assert_eq!(ep2.epoch_id, 2);
    assert_eq!(ep4.epoch_id, 4);
    assert!(ep1.checkpoint.event_count < ep2.checkpoint.event_count);
    assert_eq!(ep4.checkpoint.event_count, 308);
    assert_eq!(ep4.unique_holders, 5);
    assert!(fixture_epoch_fact(300, 250).is_err()); // 非 250 整数倍必须拒绝
}

#[test]
fn sepolia_replay_matches_fetched_ground_truth() {
    let expected: serde_json::Value =
        serde_json::from_str(include_str!("../../fixtures/sepolia_expected.json")).unwrap();
    let holder: Address = expected["sample_holder"].as_str().unwrap().parse().unwrap();
    let fact = crate::sepolia::sepolia_fact(holder).unwrap();
    assert_eq!(fact.balance, expected["sample_window_balance"].as_str().unwrap());
    assert_eq!(fact.checkpoint.event_count, expected["event_count"].as_u64().unwrap());
    assert_eq!(fact.checkpoint.event_merkle_root, expected["merkle_root"].as_str().unwrap());
    assert_eq!(fact.checkpoint.dataset_id, holderproof_types::SEPOLIA_DATASET_ID);
}

#[test]
fn dispersion_guard_filters_single_byzantine_outlier() {
    use crate::dispersion::aggregate_with_dispersion_guard;

    // 复现 NDSS 2026 论文案例：诚实节点约 $3,702.42，1 个拜占庭节点篡改为 $4,302.42
    // f = 1 (n = 4), max_dispersion_bps = 100 (1%)
    let obs = vec![
        (0u32, 370_000u128),
        (1, 370_242),
        (2, 370_500),
        (3, 430_242), // 恶意极端值
    ];
    let res = aggregate_with_dispersion_guard(&obs, 1, 100).unwrap();
    assert_eq!(res.honest_low, 370_242);
    assert_eq!(res.honest_high, 370_500);
    assert_eq!(res.median, 370_500);
    assert!(res.dispersion_bps <= 100);
}

#[test]
fn dispersion_guard_rejects_wide_spread_or_manipulation() {
    use crate::dispersion::aggregate_with_dispersion_guard;

    // 市场剧烈分叉导致诚实核心区间 [360000, 390000] 离散度达 ~7.69% > 1%
    let volatile = vec![
        (0u32, 350_000u128),
        (1, 360_000),
        (2, 390_000),
        (3, 430_242),
    ];
    assert!(aggregate_with_dispersion_guard(&volatile, 1, 100).is_err());
    // 放宽阈值至 10% (1000 bps) 时才允许通过
    assert!(aggregate_with_dispersion_guard(&volatile, 1, 1000).is_ok());
}

#[test]
fn dispersion_guard_rejects_duplicates_and_insufficient_quorum() {
    use crate::dispersion::aggregate_with_dispersion_guard;

    let dup = vec![(0u32, 370_000u128), (0, 370_100), (1, 370_200)];
    assert!(aggregate_with_dispersion_guard(&dup, 1, 100).is_err());

    let too_few = vec![(0u32, 370_000u128), (1, 370_100)];
    assert!(aggregate_with_dispersion_guard(&too_few, 1, 100).is_err());
}