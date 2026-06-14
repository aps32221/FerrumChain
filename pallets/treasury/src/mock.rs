//! 測試用模擬執行環境 / Mock runtime for `pallet-treasury-fer` unit tests and benchmarks.
#![cfg(any(test, feature = "runtime-benchmarks"))]

use crate as pallet_treasury_fer;
use frame_support::{
    derive_impl,
    traits::{ConstU64, EnsureOrigin},
};
use sp_runtime::{traits::IdentityLookup, BuildStorage};

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        Balances: pallet_balances,
        Treasury: pallet_treasury_fer,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
    type AccountId = u64;
    type Lookup = IdentityLookup<Self::AccountId>;
    type AccountData = pallet_balances::AccountData<u128>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
    type AccountStore = System;
    type Balance = u128;
    type ExistentialDeposit = ConstU64<1>;
}

/// 測試用：root 來源視為治理（§08：發行由國庫與治理控制）。
///
/// Test-only: root origin is treated as governance (§08: issuance is
/// treasury/governance-controlled).
pub struct EnsureGovernance;
impl EnsureOrigin<frame_system::pallet::RuntimeOrigin<Test>> for EnsureGovernance {
    type Success = ();

    fn try_origin(
        o: frame_system::pallet::RuntimeOrigin<Test>,
    ) -> Result<Self::Success, frame_system::pallet::RuntimeOrigin<Test>> {
        frame_system::ensure_root(o.clone()).map_err(|_| o)
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn try_successful_origin() -> Result<frame_system::pallet::RuntimeOrigin<Test>, ()> {
        Ok(frame_system::RawOrigin::Root.into())
    }
}

impl pallet_treasury_fer::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type GovernanceOrigin = EnsureGovernance;
    type WeightInfo = ();
}

/// 建立測試用初始狀態 / Build genesis storage for the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
    let t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();
    sp_io::TestExternalities::new(t)
}
