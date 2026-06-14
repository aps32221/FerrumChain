//! 測試用模擬執行環境 / Mock runtime for pallet-credential unit tests.

use crate as pallet_credential;
use frame_support::{derive_impl, traits::EnsureOrigin};
use sp_runtime::{traits::IdentityLookup, BuildStorage};

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        Credential: pallet_credential,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
    type AccountId = u64;
    type Lookup = IdentityLookup<Self::AccountId>;
}

/// 簽署來源若帳號 == 1 即視為受認可發證機構（測試用簡化邏輯）。
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

impl pallet_credential::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type IssuerOrigin = IssuerOrigin;
    type WeightInfo = ();
}

/// 建立預設測試外部環境 / Build a default test externalities.
pub fn new_test_ext() -> sp_io::TestExternalities {
    let storage = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();
    storage.into()
}
