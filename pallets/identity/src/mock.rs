//! 測試用模擬執行環境 / Mock runtime for pallet-identity-fer unit tests.

use crate as pallet_identity_fer;
use ferrum_primitives::MAX_TAG_LEN;
use frame_support::{derive_impl, traits::EnsureOrigin, BoundedVec};
use sp_core::ConstU32;
use sp_runtime::{traits::IdentityLookup, BuildStorage};

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        Identity: pallet_identity_fer,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
    type AccountId = u64;
    type Lookup = IdentityLookup<Self::AccountId>;
}

/// 簽署來源若帳號 == 1 即視為受認證簽發機構（測試用簡化邏輯）。
/// Test-only origin: account `1` is the accredited issuer.
pub struct IssuerOrigin;
impl EnsureOrigin<frame_system::pallet_prelude::OriginFor<Test>> for IssuerOrigin {
    type Success = u64;

    fn try_origin(
        o: frame_system::pallet_prelude::OriginFor<Test>,
    ) -> Result<Self::Success, frame_system::pallet_prelude::OriginFor<Test>> {
        use frame_system::RawOrigin;
        match o.clone().into() {
            Ok(RawOrigin::Signed(who)) if who == 1 => Ok(who),
            _ => Err(o),
        }
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn try_successful_origin() -> Result<frame_system::pallet_prelude::OriginFor<Test>, ()> {
        Ok(frame_system::RawOrigin::Signed(1).into())
    }
}

/// 簽署來源若帳號 == root (account 0 via Root) 即視為治理來源（測試用簡化邏輯）。
/// Test-only origin: `Root` is chain governance.
pub struct GovernanceOrigin;
impl EnsureOrigin<frame_system::pallet_prelude::OriginFor<Test>> for GovernanceOrigin {
    type Success = ();

    fn try_origin(
        o: frame_system::pallet_prelude::OriginFor<Test>,
    ) -> Result<Self::Success, frame_system::pallet_prelude::OriginFor<Test>> {
        use frame_system::RawOrigin;
        match o.clone().into() {
            Ok(RawOrigin::Root) => Ok(()),
            _ => Err(o),
        }
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn try_successful_origin() -> Result<frame_system::pallet_prelude::OriginFor<Test>, ()> {
        Ok(frame_system::RawOrigin::Root.into())
    }
}

frame_support::parameter_types! {
    /// 本地鏈標籤 `b"tw"`（測試用）。 Local chain tag `b"tw"` (for tests).
    pub LocalChainTag: BoundedVec<u8, ConstU32<MAX_TAG_LEN>> =
        BoundedVec::try_from(b"tw".to_vec()).unwrap();
}

impl pallet_identity_fer::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type IssuerOrigin = IssuerOrigin;
    type GovernanceOrigin = GovernanceOrigin;
    type LocalChainTag = LocalChainTag;
    type WeightInfo = ();
}

/// 建立預設測試外部環境 / Build a default test externalities.
pub fn new_test_ext() -> sp_io::TestExternalities {
    let storage = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();
    storage.into()
}
