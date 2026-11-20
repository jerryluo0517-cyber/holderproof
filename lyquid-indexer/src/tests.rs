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
fn committee_requires_three_distinct_members() {
    assert!(crate::models::valid_committee(&[0, 1, 2]));
    assert!(!crate::models::valid_committee(&[0, 1]));
    assert!(!crate::models::valid_committee(&[0, 1, 1]));
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
