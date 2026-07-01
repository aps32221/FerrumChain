//! 測試用模擬執行環境 / Mock runtime for `pallet-tax` unit tests and benchmarks.
#![cfg(any(test, feature = "runtime-benchmarks"))]

use crate as pallet_tax;
use crate::pallet::TreasurySettle;
use ferrum_primitives::{AccountId, FiatAmount, Hash32};
use frame_support::{derive_impl, traits::EnsureOrigin};
use sp_runtime::{traits::IdentityLookup, BuildStorage, DispatchResult};

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        Tax: pallet_tax,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
    type AccountId = AccountId;
    type Lookup = IdentityLookup<Self::AccountId>;
}

/// 測試用簽署帳戶（32 位元組皆為 `1`）。Test signer account (all-`1` 32-byte id).
pub fn alice() -> AccountId {
    AccountId::from([1u8; 32])
}

/// 測試用授權稽核員帳戶（32 位元組皆為 `9`）。
///
/// Test-only authorized-auditor account (all-`9` 32-byte id).
pub fn auditor() -> AccountId {
    AccountId::from([9u8; 32])
}

/// 測試用國庫結算實作：總是成功，不轉移任何代幣（FER 與義務金額分離，§08）。
///
/// Test-only treasury settlement: always succeeds, no token transfer (the
/// fiat obligation is settled in eTWD off the network-token ledger, §08).
pub struct MockTreasury;
impl TreasurySettle<AccountId> for MockTreasury {
    fn settle_fiat(_payer: &AccountId, _receipt: Hash32, _amount: FiatAmount) -> DispatchResult {
        Ok(())
    }
}

/// 測試用：`auditor()` 簽署來源視為授權稽核員。
///
/// Test-only: the `auditor()` signed origin is treated as an authorized
/// auditor.
pub struct EnsureAuditor;
impl EnsureOrigin<frame_system::pallet_prelude::OriginFor<Test>> for EnsureAuditor {
    type Success = AccountId;

    fn try_origin(
        o: frame_system::pallet_prelude::OriginFor<Test>,
    ) -> Result<Self::Success, frame_system::pallet_prelude::OriginFor<Test>> {
        use frame_system::RawOrigin;
        match o.clone().into() {
            Ok(RawOrigin::Signed(who)) if who == auditor() => Ok(who),
            _ => Err(o),
        }
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn try_successful_origin() -> Result<frame_system::pallet_prelude::OriginFor<Test>, ()> {
        Ok(frame_system::RawOrigin::Signed(auditor()).into())
    }
}

/// 測試用：root 來源視為治理。
///
/// Test-only: root origin is treated as governance.
pub struct EnsureGovernance;
impl EnsureOrigin<frame_system::pallet_prelude::OriginFor<Test>> for EnsureGovernance {
    type Success = ();

    fn try_origin(
        o: frame_system::pallet_prelude::OriginFor<Test>,
    ) -> Result<Self::Success, frame_system::pallet_prelude::OriginFor<Test>> {
        frame_system::ensure_root(o.clone()).map_err(|_| o)
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn try_successful_origin() -> Result<frame_system::pallet_prelude::OriginFor<Test>, ()> {
        Ok(frame_system::RawOrigin::Root.into())
    }
}

impl pallet_tax::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Treasury = MockTreasury;
    type RevenueHook = ();
    type AuditorOrigin = EnsureAuditor;
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
