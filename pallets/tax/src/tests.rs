//! 單元測試 / Unit tests for `pallet-tax` against the mock runtime.
#![cfg(test)]

use crate::{mock::*, pallet::*, Error};
use ferrum_primitives::{
    Did, FiatAmount, InvoiceAnchor, TaxKind, TaxObligation,
};
use frame_support::{assert_noop, assert_ok, BoundedVec};
use sp_std::vec::Vec;

fn sample_did() -> Did {
    Did {
        chain_tag: BoundedVec::try_from(b"tw".to_vec()).unwrap(),
        id: BoundedVec::try_from(b"alice-tax-id".to_vec()).unwrap(),
    }
}

fn sample_invoice(hash: [u8; 32]) -> InvoiceAnchor {
    InvoiceAnchor {
        invoice_hash: hash,
        issuer: 1u64,
        kind: TaxKind::ValueAdded,
        anchored_at: 1_700_000_000_000,
    }
}

fn sample_obligation(subject: Did, settled: bool) -> TaxObligation {
    TaxObligation {
        subject,
        kind: TaxKind::Income,
        amount_due: FiatAmount { currency: *b"TWD", minor_units: 123_456 },
        detail_commitment: [7u8; 32],
        settled,
    }
}

#[test]
fn anchor_invoice_works() {
    new_test_ext().execute_with(|| {
        let anchor = sample_invoice([1u8; 32]);
        assert_ok!(Tax::anchor_invoice(RuntimeOrigin::signed(1), anchor.clone()));
        assert_eq!(Invoices::<Test>::get([1u8; 32]), Some(anchor));
    });
}

#[test]
fn anchor_invoice_rejects_duplicate() {
    new_test_ext().execute_with(|| {
        let anchor = sample_invoice([2u8; 32]);
        assert_ok!(Tax::anchor_invoice(RuntimeOrigin::signed(1), anchor.clone()));
        assert_noop!(
            Tax::anchor_invoice(RuntimeOrigin::signed(1), anchor),
            Error::<Test>::InvoiceAlreadyAnchored
        );
    });
}

#[test]
fn withhold_emits_event() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let subject = sample_did();
        let amount = FiatAmount { currency: *b"TWD", minor_units: 5_000 };
        assert_ok!(Tax::withhold(RuntimeOrigin::signed(1), subject.clone(), TaxKind::Wage, amount));

        System::assert_last_event(
            Event::Withheld { subject, kind: TaxKind::Wage, amount }.into(),
        );
    });
}

#[test]
fn file_obligation_stores_record() {
    new_test_ext().execute_with(|| {
        System::set_block_number(10);
        let subject = sample_did();
        let obligation = sample_obligation(subject.clone(), false);
        assert_ok!(Tax::file_obligation(RuntimeOrigin::signed(1), obligation.clone()));

        let stored = Obligations::<Test>::get((subject, 10u64)).unwrap();
        assert_eq!(stored.amount_due, obligation.amount_due);
        assert!(!stored.settled);
    });
}

#[test]
fn settle_pays_and_marks_settled() {
    new_test_ext().execute_with(|| {
        System::set_block_number(5);
        let subject = sample_did();
        let obligation = sample_obligation(subject.clone(), false);
        assert_ok!(Tax::file_obligation(RuntimeOrigin::signed(1), obligation.clone()));

        assert_ok!(Tax::settle(RuntimeOrigin::signed(1), subject.clone(), 5));

        let stored = Obligations::<Test>::get((subject, 5u64)).unwrap();
        assert!(stored.settled);
    });
}

#[test]
fn settle_fails_if_not_found() {
    new_test_ext().execute_with(|| {
        let subject = sample_did();
        assert_noop!(
            Tax::settle(RuntimeOrigin::signed(1), subject, 99),
            Error::<Test>::ObligationNotFound
        );
    });
}

#[test]
fn settle_fails_if_already_settled() {
    new_test_ext().execute_with(|| {
        System::set_block_number(7);
        let subject = sample_did();
        let obligation = sample_obligation(subject.clone(), false);
        assert_ok!(Tax::file_obligation(RuntimeOrigin::signed(1), obligation));
        assert_ok!(Tax::settle(RuntimeOrigin::signed(1), subject.clone(), 7));
        assert_noop!(
            Tax::settle(RuntimeOrigin::signed(1), subject, 7),
            Error::<Test>::AlreadySettled
        );
    });
}

#[test]
fn authorize_audit_requires_existing_invoice() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Tax::authorize_audit(RuntimeOrigin::signed(1), [9u8; 32], [8u8; 32]),
            Error::<Test>::InvoiceNotFound
        );
    });
}

#[test]
fn authorize_audit_records_commitment() {
    new_test_ext().execute_with(|| {
        let anchor = sample_invoice([3u8; 32]);
        assert_ok!(Tax::anchor_invoice(RuntimeOrigin::signed(1), anchor));
        assert_ok!(Tax::authorize_audit(RuntimeOrigin::signed(1), [3u8; 32], [4u8; 32]));
        assert_eq!(AuditLog::<Test>::get([3u8; 32]), Some([4u8; 32]));
    });
}

#[test]
fn set_brackets_requires_governance() {
    new_test_ext().execute_with(|| {
        let brackets = BoundedVec::try_from(Vec::new()).unwrap();
        assert_noop!(
            Tax::set_brackets(RuntimeOrigin::signed(1), brackets.clone()),
            sp_runtime::DispatchError::BadOrigin
        );
        assert_ok!(Tax::set_brackets(RuntimeOrigin::root(), brackets));
    });
}
