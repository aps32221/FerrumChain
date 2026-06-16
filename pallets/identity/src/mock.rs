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
    type AccountId = ferrum_primitives::AccountId;
    type Lookup = IdentityLookup<Self::AccountId>;
}

/// 測試帳戶：以固定的 32-byte 帳戶 id 模擬簽發機構與一般使用者。
/// Test accounts: fixed 32-byte account ids standing in for issuer / users.
pub fn account(byte: u8) -> ferrum_primitives::AccountId {
    ferrum_primitives::AccountId::from([byte; 32])
}

/// 簽署來源若帳號 == `account(1)` 即視為受認證簽發機構（測試用簡化邏輯）。
/// Test-only origin: `account(1)` is the accredited issuer.
pub struct IssuerOrigin;
impl EnsureOrigin<frame_system::pallet_prelude::OriginFor<Test>> for IssuerOrigin {
    type Success = ferrum_primitives::AccountId;

    fn try_origin(
        o: frame_system::pallet_prelude::OriginFor<Test>,
    ) -> Result<Self::Success, frame_system::pallet_prelude::OriginFor<Test>> {
        use frame_system::RawOrigin;
        match o.clone().into() {
            Ok(RawOrigin::Signed(who)) if who == account(1) => Ok(who),
            _ => Err(o),
        }
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn try_successful_origin() -> Result<frame_system::pallet_prelude::OriginFor<Test>, ()> {
        Ok(frame_system::RawOrigin::Signed(account(1)).into())
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
///
/// 將區塊號設為 1，使 `frame_system` 開始記錄事件（區塊 0 不記錄事件）。
/// Sets the block number to 1 so `frame_system` records events (block 0
/// does not emit events).
pub fn new_test_ext() -> sp_io::TestExternalities {
    let storage = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();
    let mut ext: sp_io::TestExternalities = storage.into();
    ext.execute_with(|| System::set_block_number(1));
    ext
}
