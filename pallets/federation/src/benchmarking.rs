//! 基準測試 / Benchmarking stub for `pallet-federation` using `frame_benchmarking::v2`.
#![cfg(feature = "runtime-benchmarks")]

use super::*;
use crate::pallet::{
    BasketWeights, Config, Members, NextId, Pallet, Proposal, Proposals, Queued,
};
use ferrum_primitives::{FederationAction, MemberId, Vote};
use frame_benchmarking::v2::*;
use frame_support::BoundedVec;
use frame_system::RawOrigin;
use sp_runtime::Perbill;

const TW: MemberId = *b"TW";
const JP: MemberId = *b"JP";
const US: MemberId = *b"US";

fn seed_council<T: Config>() {
    for m in [TW, JP, US] {
        Members::<T>::insert(m, true);
    }
    let mut weights: BoundedVec<(MemberId, Perbill), frame_support::traits::ConstU32<{ ferrum_primitives::MAX_FEDERATION_MEMBERS }>> =
        BoundedVec::new();
    // 註:`MAX_FEDERATION_MEMBERS` 為常數,以下推入操作受其容量限制。
    // Note: pushes below are bounded by `MAX_FEDERATION_MEMBERS`.
    let _ = weights.try_push((TW, Perbill::from_percent(40)));
    let _ = weights.try_push((JP, Perbill::from_percent(35)));
    let _ = weights.try_push((US, Perbill::from_percent(25)));
    BasketWeights::<T>::put(weights);
}

#[benchmarks]
mod benchmarks {
    use super::*;

    #[benchmark]
    fn propose() {
        seed_council::<T>();
        let action = FederationAction::SetParameter {
            key: BoundedVec::try_from(b"fee".to_vec()).unwrap(),
            value: 1,
        };

        #[extrinsic_call]
        _(RawOrigin::Signed(whitelisted_caller()), action);

        assert_eq!(NextId::<T>::get(), 1);
    }

    #[benchmark]
    fn vote() {
        seed_council::<T>();
        let action = FederationAction::SetParameter {
            key: BoundedVec::try_from(b"fee".to_vec()).unwrap(),
            value: 1,
        };
        Proposals::<T>::insert(0u64, Proposal::<T>::new(action, frame_system::Pallet::<T>::block_number()));

        #[extrinsic_call]
        _(RawOrigin::Signed(whitelisted_caller()), 0u64, Vote::Aye);

        assert!(Proposals::<T>::get(0u64).unwrap().votes.len() > 0);
    }

    #[benchmark]
    fn close() {
        seed_council::<T>();
        let action = FederationAction::SetParameter {
            key: BoundedVec::try_from(b"fee".to_vec()).unwrap(),
            value: 1,
        };
        let mut p = Proposal::<T>::new(action, frame_system::Pallet::<T>::block_number());
        let _ = p.votes.try_push((TW, Vote::Aye));
        let _ = p.votes.try_push((JP, Vote::Aye));
        let _ = p.votes.try_push((US, Vote::Aye));
        Proposals::<T>::insert(0u64, p);

        #[extrinsic_call]
        _(RawOrigin::Signed(whitelisted_caller()), 0u64);

        assert!(Queued::<T>::iter().next().is_some());
    }

    #[benchmark]
    fn set_membership() {
        #[extrinsic_call]
        _(RawOrigin::Signed(whitelisted_caller()), TW, true);

        assert!(Members::<T>::get(TW));
    }

    #[benchmark]
    fn set_basket() {
        let mut entries = BoundedVec::new();
        let _ = entries.try_push(ferrum_primitives::BasketEntry { cbdc: *b"USD", weight: Perbill::from_percent(100) });
        let basket = ferrum_primitives::XsuBasket { entries, version: 1 };

        #[extrinsic_call]
        _(RawOrigin::Signed(whitelisted_caller()), basket);

        assert!(crate::pallet::ActiveBasket::<T>::get().is_some());
    }

    #[benchmark]
    fn mint_xsu() {
        let mut entries = BoundedVec::new();
        let _ = entries.try_push(ferrum_primitives::BasketEntry { cbdc: *b"USD", weight: Perbill::from_percent(100) });
        let basket = ferrum_primitives::XsuBasket { entries, version: 1 };
        let _ = Pallet::<T>::do_set_basket(basket);

        #[extrinsic_call]
        _(RawOrigin::Signed(whitelisted_caller()), *b"USD", 1_000u128);

        assert_eq!(crate::pallet::XsuIssued::<T>::get(), 1_000u128);
    }

    #[benchmark]
    fn redeem_xsu() {
        let mut entries = BoundedVec::new();
        let _ = entries.try_push(ferrum_primitives::BasketEntry { cbdc: *b"USD", weight: Perbill::from_percent(100) });
        let basket = ferrum_primitives::XsuBasket { entries, version: 1 };
        let _ = Pallet::<T>::do_set_basket(basket);
        let caller: T::AccountId = whitelisted_caller();
        let _ = Pallet::<T>::mint_xsu(RawOrigin::Signed(caller.clone()).into(), *b"USD", 1_000u128);

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), *b"USD", 200u128);

        assert_eq!(crate::pallet::XsuIssued::<T>::get(), 800u128);
    }

    #[benchmark]
    fn book_clearing() {
        let mut entries = BoundedVec::new();
        let _ = entries.try_push(ferrum_primitives::BasketEntry { cbdc: *b"USD", weight: Perbill::from_percent(100) });
        let basket = ferrum_primitives::XsuBasket { entries, version: 1 };
        let _ = Pallet::<T>::do_set_basket(basket);
        let caller: T::AccountId = whitelisted_caller();
        let _ = Pallet::<T>::mint_xsu(RawOrigin::Signed(caller.clone()).into(), *b"USD", 1_000u128);

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), JP, ferrum_primitives::XsuAmount(100));

        assert_eq!(crate::pallet::XsuBalances::<T>::get(JP), 100u128);
    }

    #[benchmark]
    fn net_and_settle() {
        #[extrinsic_call]
        _(RawOrigin::Signed(whitelisted_caller()), 1u32);
    }

    #[benchmark]
    fn publish_proof_of_reserve() {
        #[extrinsic_call]
        _(RawOrigin::Signed(whitelisted_caller()));

        assert!(crate::pallet::LastProofOfReserve::<T>::get().is_some());
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
