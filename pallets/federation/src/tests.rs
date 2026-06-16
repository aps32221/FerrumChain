//! 單元測試 / Unit tests for `pallet-federation` against the mock runtime.
#![cfg(test)]

use crate::{mock::*, Error, Event};
use ferrum_primitives::{CbdcCode, FederationAction, MemberId, Vote, XsuAmount, XsuBasket};
use frame_support::{assert_noop, assert_ok, traits::Hooks, BoundedVec};
use sp_runtime::Perbill;

fn member(account: u64) -> MemberId {
    account_to_member(account)
}

/// 設定四個理事會成員及其 XSU 籃子權重(40/30/20/10%)。
///
/// Seat the four council members and set their XSU basket weights
/// (40/30/20/10%).
fn seed_council() {
    for (acc, weight) in [(0u64, 40u8), (1u64, 30u8), (2u64, 20u8), (3u64, 10u8)] {
        crate::Members::<Test>::insert(member(acc), true);
        let mut weights = crate::BasketWeights::<Test>::get();
        let _ = weights.try_push((member(acc), Perbill::from_percent(weight as u32)));
        crate::BasketWeights::<Test>::put(weights);
    }
}

fn propose_param_change() -> u64 {
    assert_ok!(Federation::propose(
        frame_system::RawOrigin::Signed(0).into(),
        FederationAction::SetParameter {
            key: BoundedVec::try_from(b"fee".to_vec()).unwrap(),
            value: 42,
        },
    ));
    0
}

#[test]
fn propose_creates_proposal_and_emits_event() {
    new_test_ext().execute_with(|| {
        seed_council();
        let id = propose_param_change();
        assert!(crate::Proposals::<Test>::contains_key(id));
        System::assert_has_event(Event::Proposed { id, by: member(0) }.into());
    });
}

#[test]
fn vote_records_ballot_and_rejects_double_vote() {
    new_test_ext().execute_with(|| {
        seed_council();
        let id = propose_param_change();

        assert_ok!(Federation::vote(frame_system::RawOrigin::Signed(0).into(), id, Vote::Aye));
        assert_noop!(
            Federation::vote(frame_system::RawOrigin::Signed(0).into(), id, Vote::Nay),
            Error::<Test>::AlreadyVoted
        );
    });
}

#[test]
fn close_rejects_when_dual_majority_not_met() {
    new_test_ext().execute_with(|| {
        seed_council();
        let id = propose_param_change();

        // Only TW (40% weight) votes Aye out of 1 ballot cast; total ballots
        // == 1, ayes == 1 -> by_count 100% passes, but only one member voted
        // so by_count axis with total=1 is trivially 100%. To exercise a
        // genuine rejection, cast Nay from the proposer and Aye from a
        // minority-weight member only.
        assert_ok!(Federation::vote(frame_system::RawOrigin::Signed(3).into(), id, Vote::Aye)); // EU 10%
        assert_ok!(Federation::vote(frame_system::RawOrigin::Signed(0).into(), id, Vote::Nay)); // TW 40%
        assert_ok!(Federation::vote(frame_system::RawOrigin::Signed(1).into(), id, Vote::Nay)); // JP 30%
        assert_ok!(Federation::vote(frame_system::RawOrigin::Signed(2).into(), id, Vote::Nay)); // US 20%

        // ayes=1/4=25% < 2/3 -> rejected on both axes.
        assert_noop!(
            Federation::close(frame_system::RawOrigin::Signed(0).into(), id),
            Error::<Test>::Rejected
        );
    });
}

#[test]
fn close_queues_on_dual_majority_and_on_initialize_enacts() {
    new_test_ext().execute_with(|| {
        seed_council();
        let id = propose_param_change();

        // TW(40) + JP(30) + US(20) = 90% weight, 3/4 = 75% count >= 2/3.
        assert_ok!(Federation::vote(frame_system::RawOrigin::Signed(0).into(), id, Vote::Aye));
        assert_ok!(Federation::vote(frame_system::RawOrigin::Signed(1).into(), id, Vote::Aye));
        assert_ok!(Federation::vote(frame_system::RawOrigin::Signed(2).into(), id, Vote::Aye));
        assert_ok!(Federation::vote(frame_system::RawOrigin::Signed(3).into(), id, Vote::Nay));

        assert_ok!(Federation::close(frame_system::RawOrigin::Signed(0).into(), id));

        let eta = System::block_number() + TestTimelock::get();
        assert_eq!(crate::Queued::<Test>::get(eta), Some(id));

        // Advance to eta and run on_initialize.
        System::set_block_number(eta);
        Federation::on_initialize(eta);

        assert!(crate::Proposals::<Test>::get(id).is_none());
        System::assert_has_event(Event::Enacted { id }.into());
    });
}

#[test]
fn admit_member_action_seats_new_member_on_enact() {
    new_test_ext().execute_with(|| {
        seed_council();

        assert_ok!(Federation::propose(
            frame_system::RawOrigin::Signed(0).into(),
            FederationAction::AdmitMember { member: *b"CN" },
        ));
        let id = 0;

        // Membership domain needs 3/4 — all four members vote Aye.
        for acc in 0..4u64 {
            assert_ok!(Federation::vote(frame_system::RawOrigin::Signed(acc).into(), id, Vote::Aye));
        }
        assert_ok!(Federation::close(frame_system::RawOrigin::Signed(0).into(), id));

        let eta = System::block_number() + TestTimelock::get();
        System::set_block_number(eta);
        Federation::on_initialize(eta);

        assert!(crate::Members::<Test>::get(*b"CN"));
    });
}

#[test]
fn set_basket_requires_balanced_weights() {
    new_test_ext().execute_with(|| {
        seed_council();

        let mut entries = BoundedVec::new();
        entries
            .try_push(ferrum_primitives::BasketEntry { cbdc: *b"USD", weight: Perbill::from_percent(50) })
            .unwrap();
        // Intentionally unbalanced (only 50%).
        let basket = XsuBasket { entries, version: 1 };

        assert_noop!(
            Federation::set_basket(frame_system::RawOrigin::Signed(0).into(), basket),
            Error::<Test>::UnbalancedBasket
        );
    });
}

#[test]
fn mint_and_redeem_xsu_round_trip() {
    new_test_ext().execute_with(|| {
        seed_council();

        let mut entries = BoundedVec::new();
        entries
            .try_push(ferrum_primitives::BasketEntry { cbdc: *b"USD", weight: Perbill::from_percent(100) })
            .unwrap();
        let basket = XsuBasket { entries, version: 1 };
        assert_ok!(Federation::set_basket(frame_system::RawOrigin::Signed(0).into(), basket));

        let cbdc: CbdcCode = *b"USD";
        assert_ok!(Federation::mint_xsu(frame_system::RawOrigin::Signed(0).into(), cbdc, 1_000));
        assert_eq!(crate::XsuIssued::<Test>::get(), 1_000);
        assert_eq!(crate::XsuBalances::<Test>::get(member(0)), 1_000);
        assert_eq!(crate::ReservePool::<Test>::get(cbdc), 1_000);

        assert_ok!(Federation::redeem_xsu(frame_system::RawOrigin::Signed(0).into(), cbdc, 400));
        assert_eq!(crate::XsuIssued::<Test>::get(), 600);
        assert_eq!(crate::XsuBalances::<Test>::get(member(0)), 600);
        assert_eq!(crate::ReservePool::<Test>::get(cbdc), 600);
    });
}

#[test]
fn book_clearing_moves_net_position_and_settle_emits_event() {
    new_test_ext().execute_with(|| {
        seed_council();

        let mut entries = BoundedVec::new();
        entries
            .try_push(ferrum_primitives::BasketEntry { cbdc: *b"USD", weight: Perbill::from_percent(100) })
            .unwrap();
        let basket = XsuBasket { entries, version: 1 };
        assert_ok!(Federation::set_basket(frame_system::RawOrigin::Signed(0).into(), basket));

        let cbdc: CbdcCode = *b"USD";
        assert_ok!(Federation::mint_xsu(frame_system::RawOrigin::Signed(0).into(), cbdc, 1_000));

        // TW (account 0) clears 200 XSU to JP (member of account 1).
        assert_ok!(Federation::book_clearing(
            frame_system::RawOrigin::Signed(0).into(),
            member(1),
            XsuAmount(200),
        ));
        assert_eq!(crate::XsuBalances::<Test>::get(member(0)), 800);
        assert_eq!(crate::XsuBalances::<Test>::get(member(1)), 200);

        assert_ok!(Federation::net_and_settle(frame_system::RawOrigin::Signed(0).into(), 1));
        System::assert_has_event(Event::NetSettled { window: 1 }.into());
    });
}

#[test]
fn publish_proof_of_reserve_records_digest() {
    new_test_ext().execute_with(|| {
        seed_council();

        let mut entries = BoundedVec::new();
        entries
            .try_push(ferrum_primitives::BasketEntry { cbdc: *b"USD", weight: Perbill::from_percent(100) })
            .unwrap();
        let basket = XsuBasket { entries, version: 1 };
        assert_ok!(Federation::set_basket(frame_system::RawOrigin::Signed(0).into(), basket));

        let cbdc: CbdcCode = *b"USD";
        assert_ok!(Federation::mint_xsu(frame_system::RawOrigin::Signed(0).into(), cbdc, 1_000));

        assert_ok!(Federation::publish_proof_of_reserve(frame_system::RawOrigin::Signed(0).into()));
        assert!(crate::LastProofOfReserve::<Test>::get().is_some());
    });
}
