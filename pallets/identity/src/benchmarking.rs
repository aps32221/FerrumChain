//! Benchmarking stub for pallet-identity-fer (frame_benchmarking::v2).
//! pallet-identity-fer 的基準測試骨架。

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use crate::pallet::{Call, Config, Dids, Pallet, RevocationAccumulator};
use ferrum_primitives::{Did, DidDocument};
use frame_benchmarking::v2::*;
use frame_support::BoundedVec;
use frame_system::RawOrigin;

fn bench_did() -> Did {
    Did {
        chain_tag: BoundedVec::try_from(b"tw".to_vec()).unwrap(),
        id: BoundedVec::try_from(b"bench-subject".to_vec()).unwrap(),
    }
}

#[benchmarks]
mod benchmarks {
    use super::*;

    #[benchmark]
    fn anchor_did() {
        let issuer = T::IssuerOrigin::try_successful_origin().unwrap();
        let controller: T::AccountId = whitelisted_caller();
        let doc = DidDocument {
            did: bench_did(),
            controller,
            doc_hash: [1u8; 32],
            keys: BoundedVec::default(),
            revocation_commitment: [0u8; 32],
            anchored_at: 0u32.into(),
        };

        #[extrinsic_call]
        _(issuer as T::RuntimeOrigin, doc);

        assert!(Dids::<T>::contains_key(bench_did()));
    }

    #[benchmark]
    fn rotate_keys() {
        let controller: T::AccountId = whitelisted_caller();
        let doc = DidDocument {
            did: bench_did(),
            controller: controller.clone(),
            doc_hash: [2u8; 32],
            keys: BoundedVec::default(),
            revocation_commitment: [0u8; 32],
            anchored_at: 0u32.into(),
        };
        Dids::<T>::insert(bench_did(), &doc);

        let keys: BoundedVec<_, _> = BoundedVec::default();

        #[extrinsic_call]
        _(RawOrigin::Signed(controller), bench_did(), keys);
    }

    #[benchmark]
    fn update_revocation() {
        let issuer = T::IssuerOrigin::try_successful_origin().unwrap();

        #[extrinsic_call]
        _(issuer as T::RuntimeOrigin, [3u8; 32]);

        assert_eq!(RevocationAccumulator::<T>::get(), [3u8; 32]);
    }

    #[benchmark]
    fn register_issuer() {
        let gov = T::GovernanceOrigin::try_successful_origin().unwrap();
        let who: T::AccountId = whitelisted_caller();

        #[extrinsic_call]
        _(gov as T::RuntimeOrigin, who.clone());

        assert!(Pallet::<T>::is_accredited_issuer(&who));
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
