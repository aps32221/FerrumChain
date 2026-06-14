//! 單元測試 / Unit tests for pallet-credential.

use crate::{mock::*, Error, Event};
use ferrum_primitives::{CredentialAnchor, CredentialKind, CredentialStatus, Did};
use frame_support::{assert_noop, assert_ok, BoundedVec};

fn test_subject() -> Did {
    Did {
        chain_tag: BoundedVec::try_from(b"tw".to_vec()).unwrap(),
        id: BoundedVec::try_from(b"subject-001".to_vec()).unwrap(),
    }
}

fn test_anchor(payload_hash: [u8; 32]) -> CredentialAnchor {
    CredentialAnchor {
        subject: test_subject(),
        issuer: 1u64,
        kind: CredentialKind::Age,
        payload_hash,
        status: CredentialStatus::Active,
        expires_at: None,
    }
}

#[test]
fn issue_works_and_emits_event() {
    new_test_ext().execute_with(|| {
        let anchor = test_anchor([1u8; 32]);

        assert_ok!(Credential::issue(RuntimeOrigin::signed(1), anchor.clone()));

        assert_eq!(Credential::credentials([1u8; 32]).unwrap(), anchor);
        assert_eq!(Credential::by_subject(test_subject()).into_inner(), vec![[1u8; 32]]);

        System::assert_last_event(
            Event::CredentialIssued {
                subject: test_subject(),
                issuer: 1u64,
                payload_hash: [1u8; 32],
            }
            .into(),
        );
    });
}

#[test]
fn issue_rejects_non_issuer_origin() {
    new_test_ext().execute_with(|| {
        let anchor = test_anchor([2u8; 32]);
        assert_noop!(
            Credential::issue(RuntimeOrigin::signed(2), anchor),
            frame_support::error::BadOrigin
        );
    });
}

#[test]
fn issue_rejects_duplicate_payload_hash() {
    new_test_ext().execute_with(|| {
        let anchor = test_anchor([3u8; 32]);
        assert_ok!(Credential::issue(RuntimeOrigin::signed(1), anchor.clone()));
        assert_noop!(
            Credential::issue(RuntimeOrigin::signed(1), anchor),
            Error::<Test>::CredentialAlreadyExists
        );
    });
}

#[test]
fn revoke_works() {
    new_test_ext().execute_with(|| {
        let anchor = test_anchor([4u8; 32]);
        assert_ok!(Credential::issue(RuntimeOrigin::signed(1), anchor));

        assert_ok!(Credential::revoke(RuntimeOrigin::signed(1), [4u8; 32]));

        assert_eq!(
            Credential::credentials([4u8; 32]).unwrap().status,
            CredentialStatus::Revoked
        );
        System::assert_last_event(Event::CredentialRevoked { payload_hash: [4u8; 32] }.into());
    });
}

#[test]
fn revoke_unknown_credential_fails() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Credential::revoke(RuntimeOrigin::signed(1), [9u8; 32]),
            Error::<Test>::CredentialNotFound
        );
    });
}

#[test]
fn set_status_works() {
    new_test_ext().execute_with(|| {
        let anchor = test_anchor([5u8; 32]);
        assert_ok!(Credential::issue(RuntimeOrigin::signed(1), anchor));

        assert_ok!(Credential::set_status(
            RuntimeOrigin::signed(1),
            [5u8; 32],
            CredentialStatus::Suspended
        ));

        assert_eq!(
            Credential::credentials([5u8; 32]).unwrap().status,
            CredentialStatus::Suspended
        );
        System::assert_last_event(
            Event::CredentialStatusUpdated {
                payload_hash: [5u8; 32],
                status: CredentialStatus::Suspended,
            }
            .into(),
        );
    });
}

#[test]
fn log_presentation_works_and_prevents_replay() {
    new_test_ext().execute_with(|| {
        let nullifier = [7u8; 32];
        let commitment = [8u8; 32];

        assert_ok!(Credential::log_presentation(RuntimeOrigin::signed(2), nullifier, commitment));

        assert_eq!(Credential::presentations(nullifier).unwrap(), commitment);
        System::assert_last_event(
            Event::PresentationLogged { nullifier, commitment }.into(),
        );

        // Replay with the same nullifier must fail.
        assert_noop!(
            Credential::log_presentation(RuntimeOrigin::signed(2), nullifier, commitment),
            Error::<Test>::PresentationAlreadyLogged
        );
    });
}
