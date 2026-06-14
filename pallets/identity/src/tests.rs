//! 單元測試 / Unit tests for pallet-identity-fer.

use crate::{mock::*, DidRegistry, Error, Event};
use ferrum_primitives::{Did, DidDocument, DidKeyRef, KeyKind};
use frame_support::{assert_noop, assert_ok, BoundedVec};

fn test_did() -> Did {
    Did {
        chain_tag: BoundedVec::try_from(b"tw".to_vec()).unwrap(),
        id: BoundedVec::try_from(b"subject-001".to_vec()).unwrap(),
    }
}

fn foreign_did() -> Did {
    Did {
        chain_tag: BoundedVec::try_from(b"jp".to_vec()).unwrap(),
        id: BoundedVec::try_from(b"subject-jp".to_vec()).unwrap(),
    }
}

fn test_doc(did: Did, controller: u64, doc_hash: [u8; 32]) -> DidDocument {
    DidDocument {
        did,
        controller,
        doc_hash,
        keys: BoundedVec::default(),
        revocation_commitment: [0u8; 32],
        anchored_at: 0,
    }
}

#[test]
fn anchor_did_works_and_emits_event() {
    new_test_ext().execute_with(|| {
        let doc = test_doc(test_did(), 1, [1u8; 32]);

        assert_ok!(Identity::anchor_did(RuntimeOrigin::signed(1), doc.clone()));

        assert_eq!(Identity::dids(test_did()).unwrap(), doc);
        assert_eq!(Identity::did_by_controller(1u64).unwrap(), test_did());

        System::assert_last_event(
            Event::DidAnchored { did: test_did(), controller: 1u64, doc_hash: [1u8; 32] }.into(),
        );

        // Cross-pallet read-only interface should also resolve it.
        assert!(<Identity as DidRegistry>::exists(&test_did()));
    });
}

#[test]
fn anchor_did_rejects_non_issuer_origin() {
    new_test_ext().execute_with(|| {
        let doc = test_doc(test_did(), 2, [2u8; 32]);
        assert_noop!(
            Identity::anchor_did(RuntimeOrigin::signed(2), doc),
            frame_support::error::BadOrigin
        );
    });
}

#[test]
fn anchor_did_rejects_wrong_chain_tag() {
    new_test_ext().execute_with(|| {
        let doc = test_doc(foreign_did(), 1, [3u8; 32]);
        assert_noop!(
            Identity::anchor_did(RuntimeOrigin::signed(1), doc),
            Error::<Test>::WrongChainTag
        );
    });
}

#[test]
fn anchor_did_rejects_duplicate() {
    new_test_ext().execute_with(|| {
        let doc = test_doc(test_did(), 1, [4u8; 32]);
        assert_ok!(Identity::anchor_did(RuntimeOrigin::signed(1), doc.clone()));
        assert_noop!(
            Identity::anchor_did(RuntimeOrigin::signed(1), doc),
            Error::<Test>::AlreadyExists
        );
    });
}

#[test]
fn rotate_keys_works() {
    new_test_ext().execute_with(|| {
        let doc = test_doc(test_did(), 1, [5u8; 32]);
        assert_ok!(Identity::anchor_did(RuntimeOrigin::signed(1), doc));

        let keys: BoundedVec<DidKeyRef, _> = BoundedVec::try_from(vec![DidKeyRef {
            kind: KeyKind::Sr25519,
            key_hash: [9u8; 32],
        }])
        .unwrap();

        assert_ok!(Identity::rotate_keys(RuntimeOrigin::signed(1), test_did(), keys.clone()));

        assert_eq!(Identity::dids(test_did()).unwrap().keys, keys);
        System::assert_last_event(Event::KeysRotated { did: test_did(), key_count: 1 }.into());
    });
}

#[test]
fn rotate_keys_rejects_non_controller() {
    new_test_ext().execute_with(|| {
        let doc = test_doc(test_did(), 1, [6u8; 32]);
        assert_ok!(Identity::anchor_did(RuntimeOrigin::signed(1), doc));

        let keys: BoundedVec<DidKeyRef, _> = BoundedVec::default();
        assert_noop!(
            Identity::rotate_keys(RuntimeOrigin::signed(2), test_did(), keys),
            Error::<Test>::NotController
        );
    });
}

#[test]
fn rotate_keys_rejects_unknown_did() {
    new_test_ext().execute_with(|| {
        let keys: BoundedVec<DidKeyRef, _> = BoundedVec::default();
        assert_noop!(
            Identity::rotate_keys(RuntimeOrigin::signed(1), test_did(), keys),
            Error::<Test>::NotFound
        );
    });
}

#[test]
fn update_revocation_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(Identity::update_revocation(RuntimeOrigin::signed(1), [7u8; 32]));

        assert_eq!(Identity::revocation_accumulator(), [7u8; 32]);
        assert_eq!(<Identity as DidRegistry>::revocation_accumulator(), [7u8; 32]);
        System::assert_last_event(Event::RevocationUpdated { commitment: [7u8; 32] }.into());
    });
}

#[test]
fn update_revocation_rejects_non_issuer() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Identity::update_revocation(RuntimeOrigin::signed(2), [8u8; 32]),
            frame_support::error::BadOrigin
        );
    });
}

#[test]
fn register_issuer_works_via_governance() {
    new_test_ext().execute_with(|| {
        assert_ok!(Identity::register_issuer(RuntimeOrigin::root(), 42u64));

        assert!(Identity::is_accredited_issuer(&42u64));
        System::assert_last_event(Event::IssuerRegistered { who: 42u64 }.into());
    });
}

#[test]
fn register_issuer_rejects_non_governance() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Identity::register_issuer(RuntimeOrigin::signed(1), 42u64),
            frame_support::error::BadOrigin
        );
    });
}
