//! 基準測試 / Benchmarking stub for `pallet-tax` using `frame_benchmarking::v2`.
#![cfg(feature = "runtime-benchmarks")]

use super::*;
use crate::pallet::{Brackets, Config, Invoices, Pallet};
use ferrum_primitives::{Did, FiatAmount, InvoiceAnchor, TaxKind, TaxObligation};
use frame_benchmarking::v2::*;
use frame_support::BoundedVec;
use frame_system::RawOrigin;
use sp_std::vec::Vec;

fn sample_did() -> Did {
    Did {
        chain_tag: BoundedVec::try_from(b"tw".to_vec()).unwrap(),
        id: BoundedVec::try_from(b"bench-subject".to_vec()).unwrap(),
    }
}

#[benchmarks]
mod benchmarks {
    use super::*;

    #[benchmark]
    fn anchor_invoice() {
        let caller: T::AccountId = whitelisted_caller();
        let anchor = InvoiceAnchor {
            invoice_hash: [1u8; 32],
            issuer: caller.clone(),
            kind: TaxKind::ValueAdded,
            anchored_at: 0,
        };

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), anchor);

        assert!(Invoices::<T>::contains_key([1u8; 32]));
    }

    #[benchmark]
    fn withhold() {
        let caller: T::AccountId = whitelisted_caller();
        let subject = sample_did();
        let amount = FiatAmount { currency: *b"TWD", minor_units: 1_000 };

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), subject, TaxKind::Wage, amount);
    }

    #[benchmark]
    fn file_obligation() {
        let caller: T::AccountId = whitelisted_caller();
        let obligation = TaxObligation {
            subject: sample_did(),
            kind: TaxKind::Income,
            amount_due: FiatAmount { currency: *b"TWD", minor_units: 1_000 },
            detail_commitment: [2u8; 32],
            settled: false,
        };

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), obligation);
    }

    #[benchmark]
    fn set_brackets() {
        let brackets: BoundedVec<_, _> = BoundedVec::try_from(Vec::new()).unwrap();

        #[extrinsic_call]
        _(RawOrigin::Root, brackets);

        assert!(Brackets::<T>::get().is_empty());
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
