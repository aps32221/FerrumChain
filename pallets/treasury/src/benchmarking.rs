//! 基準測試 / Benchmarking stub for `pallet-treasury-fer` using `frame_benchmarking::v2`.
#![cfg(feature = "runtime-benchmarks")]

use super::*;
use crate::pallet::{
    Config, EtwdReceipts, Pallet, Pools, TotalBurned, POOL_STAKING_SECURITY, POOL_SUBSIDY,
};
use ferrum_primitives::FiatAmount;
use frame_benchmarking::v2::*;
use frame_support::traits::Currency;
use frame_system::RawOrigin;

#[benchmarks]
mod benchmarks {
    use super::*;

    #[benchmark]
    fn mint() {
        #[extrinsic_call]
        _(RawOrigin::Root, POOL_STAKING_SECURITY, 1_000u128);

        assert_eq!(Pools::<T>::get(POOL_STAKING_SECURITY), 1_000u128);
    }

    #[benchmark]
    fn burn() {
        let caller: T::AccountId = whitelisted_caller();
        // 為呼叫者注入餘額以供銷毀 / Fund the caller so it can burn.
        let _ = T::Currency::deposit_creating(&caller, 10_000u128);

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), 1_000u128);

        assert_eq!(TotalBurned::<T>::get(), 1_000u128);
    }

    #[benchmark]
    fn subsidize() {
        let who: T::AccountId = account("subsidized", 0, 0);
        Pools::<T>::insert(POOL_SUBSIDY, 1_000u128);

        #[extrinsic_call]
        _(RawOrigin::Root, who, 500u128);

        assert_eq!(Pools::<T>::get(POOL_SUBSIDY), 500u128);
    }

    #[benchmark]
    fn record_settlement() {
        let caller: T::AccountId = whitelisted_caller();
        let receipt = [7u8; 32];
        let amount = FiatAmount { currency: *b"TWD", minor_units: 1_000 };

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), receipt, amount);

        assert!(EtwdReceipts::<T>::contains_key(receipt));
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
