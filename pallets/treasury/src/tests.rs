//! 單元測試 / Unit tests for `pallet-treasury-fer` (whitepaper §08).
#![cfg(test)]

use crate::mock::*;
use crate::pallet::{
    Error, Event, EtwdReceipts, Pools, TotalBurned, POOL_PROTOCOL_DEV, POOL_STAKING_SECURITY,
    POOL_SUBSIDY,
};
use ferrum_primitives::FiatAmount;
use frame_support::{assert_noop, assert_ok, traits::Currency};

#[test]
fn mint_into_pool_by_governance_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(Treasury::mint(RuntimeOrigin::root(), POOL_STAKING_SECURITY, 1_000));
        assert_eq!(Pools::<Test>::get(POOL_STAKING_SECURITY), 1_000);

        System::assert_last_event(
            Event::Minted { pool: POOL_STAKING_SECURITY, amount: 1_000, new_balance: 1_000 }
                .into(),
        );
    });
}

#[test]
fn mint_rejects_unknown_pool() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Treasury::mint(RuntimeOrigin::root(), 99, 1_000),
            Error::<Test>::UnknownPool
        );
    });
}

#[test]
fn mint_rejects_non_governance_origin() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Treasury::mint(RuntimeOrigin::signed(1), POOL_STAKING_SECURITY, 1_000),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

#[test]
fn mint_accumulates_across_pools() {
    new_test_ext().execute_with(|| {
        assert_ok!(Treasury::mint(RuntimeOrigin::root(), POOL_PROTOCOL_DEV, 300));
        assert_ok!(Treasury::mint(RuntimeOrigin::root(), POOL_PROTOCOL_DEV, 200));
        assert_eq!(Pools::<Test>::get(POOL_PROTOCOL_DEV), 500);
    });
}

#[test]
fn burn_reduces_balance_and_tallies_total_burned() {
    new_test_ext().execute_with(|| {
        let _ = Balances::deposit_creating(&1, 10_000);

        assert_ok!(Treasury::burn(RuntimeOrigin::signed(1), 1_000));

        assert_eq!(Balances::free_balance(1), 9_000);
        assert_eq!(TotalBurned::<Test>::get(), 1_000);

        System::assert_last_event(
            Event::Burned { who: 1, amount: 1_000, total_burned: 1_000 }.into(),
        );
    });
}

#[test]
fn burn_accumulates_total_burned() {
    new_test_ext().execute_with(|| {
        let _ = Balances::deposit_creating(&1, 10_000);

        assert_ok!(Treasury::burn(RuntimeOrigin::signed(1), 500));
        assert_ok!(Treasury::burn(RuntimeOrigin::signed(1), 500));

        assert_eq!(TotalBurned::<Test>::get(), 1_000);
    });
}

#[test]
fn burn_fails_with_insufficient_balance() {
    new_test_ext().execute_with(|| {
        let _ = Balances::deposit_creating(&1, 100);

        assert_noop!(
            Treasury::burn(RuntimeOrigin::signed(1), 1_000),
            Error::<Test>::InsufficientBalance
        );
    });
}

#[test]
fn subsidize_pays_from_subsidy_pool() {
    new_test_ext().execute_with(|| {
        assert_ok!(Treasury::mint(RuntimeOrigin::root(), POOL_SUBSIDY, 1_000));

        assert_ok!(Treasury::subsidize(RuntimeOrigin::root(), 42, 400));

        assert_eq!(Pools::<Test>::get(POOL_SUBSIDY), 600);
        assert_eq!(Balances::free_balance(42), 400);

        System::assert_last_event(
            Event::Subsidized { pool: POOL_SUBSIDY, who: 42, amount: 400 }.into(),
        );
    });
}

#[test]
fn subsidize_fails_when_pool_underfunded() {
    new_test_ext().execute_with(|| {
        assert_ok!(Treasury::mint(RuntimeOrigin::root(), POOL_SUBSIDY, 100));

        assert_noop!(
            Treasury::subsidize(RuntimeOrigin::root(), 42, 400),
            Error::<Test>::InsufficientPoolBalance
        );

        // 池餘額未變更 / pool balance unchanged on failure.
        assert_eq!(Pools::<Test>::get(POOL_SUBSIDY), 100);
    });
}

#[test]
fn subsidize_rejects_non_governance_origin() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Treasury::subsidize(RuntimeOrigin::signed(1), 42, 100),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

#[test]
fn record_settlement_anchors_receipt() {
    new_test_ext().execute_with(|| {
        let receipt = [1u8; 32];
        let amount = FiatAmount { currency: *b"TWD", minor_units: 5_000 };

        assert_ok!(Treasury::record_settlement(RuntimeOrigin::signed(7), receipt, amount));

        assert_eq!(EtwdReceipts::<Test>::get(receipt), Some(amount));

        System::assert_last_event(
            Event::SettlementRecorded { payer: 7, receipt, amount }.into(),
        );
    });
}

#[test]
fn record_settlement_rejects_replay() {
    new_test_ext().execute_with(|| {
        let receipt = [2u8; 32];
        let amount = FiatAmount { currency: *b"TWD", minor_units: 1_000 };

        assert_ok!(Treasury::record_settlement(RuntimeOrigin::signed(7), receipt, amount));
        assert_noop!(
            Treasury::record_settlement(RuntimeOrigin::signed(7), receipt, amount),
            Error::<Test>::ReceiptAlreadyRecorded
        );
    });
}

#[test]
fn treasury_settle_trait_records_receipt() {
    use crate::pallet::TreasurySettle;

    new_test_ext().execute_with(|| {
        let receipt = [3u8; 32];
        let amount = FiatAmount { currency: *b"TWD", minor_units: 2_500 };

        assert_ok!(<Treasury as TreasurySettle<u64>>::settle_fiat(&9, receipt, amount));

        assert_eq!(EtwdReceipts::<Test>::get(receipt), Some(amount));
    });
}
