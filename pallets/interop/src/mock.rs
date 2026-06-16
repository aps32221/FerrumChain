//! 測試用模擬執行環境 / Mock runtime for `pallet-interop` unit tests and benchmarks.
#![cfg(any(test, feature = "runtime-benchmarks"))]

use crate as pallet_interop;
use ferrum_primitives::{Commitment, Did, DidDocument, MAX_TAG_LEN};
use frame_support::{
    derive_impl, parameter_types, traits::EnsureOrigin, BoundedVec, pallet_prelude::ConstU32,
};
use pallet_identity_fer::DidRegistry;
use sp_runtime::{traits::IdentityLookup, BuildStorage};

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        Interop: pallet_interop,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
    type AccountId = u64;
    type Lookup = IdentityLookup<Self::AccountId>;
}

/// 測試用：root 來源視為聯邦治理（條約理事會）— §11.1。
///
/// Test-only: root origin is treated as the federation-governed origin
/// (treaty council) — §11.1.
pub struct EnsureFederation;
impl EnsureOrigin<RuntimeOrigin> for EnsureFederation {
    type Success = ();

    fn try_origin(o: RuntimeOrigin) -> Result<Self::Success, RuntimeOrigin> {
        frame_system::ensure_root(o.clone()).map_err(|_| o)
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn try_successful_origin() -> Result<RuntimeOrigin, ()> {
        Ok(frame_system::RawOrigin::Root.into())
    }
}

/// 測試用：任何已簽署帳戶皆視為中繼者（§09 中繼者）。
///
/// Test-only: any signed account is treated as a relayer (§09 relayer).
pub struct EnsureRelayer;
impl EnsureOrigin<RuntimeOrigin> for EnsureRelayer {
    type Success = u64;

    fn try_origin(o: RuntimeOrigin) -> Result<Self::Success, RuntimeOrigin> {
        frame_system::ensure_signed(o.clone()).map_err(|_| o)
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn try_successful_origin() -> Result<RuntimeOrigin, ()> {
        Ok(frame_system::RawOrigin::Signed(1).into())
    }
}

/// 測試用通用 DID 解析器:本鏈 DID 一律回傳「未錨定」(足以測試跨鏈解析的
/// 外鏈與 LocalUnknown 分支)。
///
/// Test-only universal DID registry: local DIDs always resolve as unanchored —
/// enough to exercise the foreign / LocalUnknown branches of `resolve_did`.
pub struct MockDidRegistry;
impl DidRegistry for MockDidRegistry {
    fn resolve(_did: &Did) -> Option<DidDocument> {
        None
    }
    fn revocation_accumulator() -> Commitment {
        [0u8; 32]
    }
}

parameter_types! {
    /// 本地鏈標籤 `did:fer:tw`。/ This chain's local tag `did:fer:tw`.
    pub LocalChainTag: BoundedVec<u8, ConstU32<MAX_TAG_LEN>> =
        BoundedVec::try_from(b"tw".to_vec()).expect("tag fits MAX_TAG_LEN; qed");
}

impl pallet_interop::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type FederationOrigin = EnsureFederation;
    type RelayerOrigin = EnsureRelayer;
    type DidRegistry = MockDidRegistry;
    type LocalChainTag = LocalChainTag;
    type WeightInfo = ();
}

/// 建立測試用初始狀態 / Build genesis storage for the mock runtime.
///
/// 將區塊高度設為 1,使 `System::events()` 能記錄事件(區塊 0 不記錄事件）。
///
/// Sets the block number to 1 so that `System::events()` records events
/// (block 0 does not record events).
pub fn new_test_ext() -> sp_io::TestExternalities {
    let t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();
    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| System::set_block_number(1));
    ext
}
