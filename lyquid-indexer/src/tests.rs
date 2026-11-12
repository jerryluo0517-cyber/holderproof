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
