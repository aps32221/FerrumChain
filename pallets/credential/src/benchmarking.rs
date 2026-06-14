//! Benchmarking stub for pallet-credential (frame_benchmarking::v2).
//! pallet-credential 的基準測試骨架。

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use crate::pallet::{Call, Config, Credentials, Pallet, Presentations};
use ferrum_primitives::{CredentialAnchor, CredentialKind, CredentialStatus, Did};
use frame_benchmarking::v2::*;
use frame_support::BoundedVec;
use frame_system::RawOrigin;

fn bench_subject() -> Did {
    Did {
        chain_tag: BoundedVec::try_from(b"tw".to_vec()).unwrap(),
        id: BoundedVec::try_from(b"bench-subject".to_vec()).unwrap(),
    }
}

#[benchmarks]
mod benchmarks {
    use super::*;

    #[benchmark]
    fn issue() {
        let issuer: T::AccountId = whitelisted_caller();
        let anchor = CredentialAnchor {
            subject: bench_subject(),
            issuer: issuer.clone(),
            kind: CredentialKind::Age,
            payload_hash: [1u8; 32],
            status: CredentialStatus::Active,
            expires_at: None,
        };

        #[extrinsic_call]
        issue(RawOrigin::Signed(issuer), anchor);

        assert!(Credentials::<T>::contains_key([1u8; 32]));
    }

    #[benchmark]
    fn revoke() {
        let issuer: T::AccountId = whitelisted_caller();
        let anchor = CredentialAnchor {
            subject: bench_subject(),
            issuer: issuer.clone(),
            kind: CredentialKind::Age,
            payload_hash: [2u8; 32],
            status: CredentialStatus::Active,
            expires_at: None,
        };
        Credentials::<T>::insert([2u8; 32], anchor);

        #[extrinsic_call]
        revoke(RawOrigin::Signed(issuer), [2u8; 32]);

        assert_eq!(Credentials::<T>::get([2u8; 32]).unwrap().status, CredentialStatus::Revoked);
    }

    #[benchmark]
    fn set_status() {
        let issuer: T::AccountId = whitelisted_caller();
        let anchor = CredentialAnchor {
            subject: bench_subject(),
            issuer: issuer.clone(),
            kind: CredentialKind::Age,
            payload_hash: [3u8; 32],
            status: CredentialStatus::Active,
            expires_at: None,
        };
        Credentials::<T>::insert([3u8; 32], anchor);

        #[extrinsic_call]
        set_status(RawOrigin::Signed(issuer), [3u8; 32], CredentialStatus::Suspended);

        assert_eq!(Credentials::<T>::get([3u8; 32]).unwrap().status, CredentialStatus::Suspended);
    }

    #[benchmark]
    fn log_presentation() {
        let caller: T::AccountId = whitelisted_caller();
        let nullifier = [4u8; 32];
        let commitment = [5u8; 32];

        #[extrinsic_call]
        log_presentation(RawOrigin::Signed(caller), nullifier, commitment);

        assert_eq!(Presentations::<T>::get(nullifier).unwrap(), commitment);
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
