//! 基準測試 / Benchmarking stub for `pallet-interop` using `frame_benchmarking::v2`.
#![cfg(feature = "runtime-benchmarks")]

use super::*;
use crate::pallet::{
    Config, Instructions, InteropValidators, NextInstruction, Pallet, TrustRegistry,
};
use ferrum_primitives::{ClearingInstruction, TrustRegistryEntry, XcmStatus, XsuAmount, MIN_VALIDATOR_BOND};
use frame_benchmarking::v2::*;
use frame_support::BoundedVec;
use frame_system::RawOrigin;

#[benchmarks]
mod benchmarks {
    use super::*;

    #[benchmark]
    fn register_issuer() {
        let entry = TrustRegistryEntry {
            country: *b"TW",
            issuer_key_hash: [1u8; 32],
            scope: BoundedVec::try_from(b"tax".to_vec()).unwrap(),
            active: true,
        };

        #[extrinsic_call]
        _(RawOrigin::Root, entry.clone());

        assert!(TrustRegistry::<T>::contains_key(entry.country, entry.issuer_key_hash));
    }

    #[benchmark]
    fn submit_instruction() {
        let caller: T::AccountId = whitelisted_caller();
        let instr = ClearingInstruction {
            from: *b"TW",
            to: *b"JP",
            amount: XsuAmount(1_000),
            detail_commitment: [0u8; 32],
            status: XcmStatus::Pending,
        };

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), instr);

        assert_eq!(NextInstruction::<T>::get(), 1);
    }

    // NOTE: `verify_finality` and `rotate_authority_set` are intentionally not
    // benchmarked here: both require a cryptographically valid GRANDPA
    // justification (ed25519 signatures over the localized payload), which
    // cannot be produced inside a no_std benchmark. Their weights are the
    // conservative manual estimates in `weights.rs` (the pairing/ed25519 cost is
    // bounded by `MAX_PRECOMMITS`). The end-to-end signature path is exercised
    // by the unit tests in `tests.rs`.

    #[benchmark]
    fn net_and_settle() {
        let instr = ClearingInstruction {
            from: *b"TW",
            to: *b"JP",
            amount: XsuAmount(1_000),
            detail_commitment: [0u8; 32],
            status: XcmStatus::FinalityVerified,
        };
        Instructions::<T>::insert(0u64, instr);

        #[extrinsic_call]
        _(RawOrigin::Root, 1u32);

        assert_eq!(Instructions::<T>::get(0u64).unwrap().status, XcmStatus::Accepted);
    }

    #[benchmark]
    fn register_validator() {
        let caller: T::AccountId = whitelisted_caller();

        #[extrinsic_call]
        _(RawOrigin::Signed(caller.clone()), MIN_VALIDATOR_BOND);

        assert_eq!(InteropValidators::<T>::get(&caller), Some(MIN_VALIDATOR_BOND));
    }

    #[benchmark]
    fn slash_validator() {
        let validator: T::AccountId = account("validator", 0, 0);
        InteropValidators::<T>::insert(&validator, MIN_VALIDATOR_BOND);

        #[extrinsic_call]
        _(RawOrigin::Root, validator.clone(), 1_000u128);

        assert_eq!(InteropValidators::<T>::get(&validator), Some(MIN_VALIDATOR_BOND - 1_000));
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
