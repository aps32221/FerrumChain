//! 測試用模擬執行環境 / Mock runtime for pallet-credential unit tests.

use crate as pallet_credential;
use ferrum_primitives::AccountId;
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
    type AccountId = AccountId;
    type Lookup = IdentityLookup<Self::AccountId>;
}

/// 受認可發證機構之測試帳戶（32 位元組皆為 `1`）。
/// The accredited-issuer test account (all-`1` 32-byte id).
pub fn issuer_account() -> AccountId {
    AccountId::from([1u8; 32])
}

/// 一般（非發證機構）測試帳戶（32 位元組皆為 `2`）。
/// A non-issuer test account (all-`2` 32-byte id).
pub fn other_account() -> AccountId {
    AccountId::from([2u8; 32])
}

/// 簽署來源若帳號 == [1u8;32] 即視為受認可發證機構（測試用簡化邏輯）。
/// Test-only origin: the all-`1` account is the accredited issuer.
pub struct IssuerOrigin;
impl EnsureOrigin<frame_system::pallet_prelude::OriginFor<Test>> for IssuerOrigin {
    type Success = AccountId;

    fn try_origin(
        o: frame_system::pallet_prelude::OriginFor<Test>,
    ) -> Result<Self::Success, frame_system::pallet_prelude::OriginFor<Test>> {
        use frame_system::RawOrigin;
        match o.clone().into() {
            Ok(RawOrigin::Signed(who)) if who == issuer_account() => Ok(who),
            _ => Err(o),
        }
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn try_successful_origin() -> Result<frame_system::pallet_prelude::OriginFor<Test>, ()> {
        Ok(frame_system::RawOrigin::Signed(issuer_account()).into())
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
