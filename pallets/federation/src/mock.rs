//! 測試用模擬執行環境 / Mock runtime for `pallet-federation` unit tests and benchmarks.
#![cfg(any(test, feature = "runtime-benchmarks"))]

use crate as pallet_federation;
use ferrum_primitives::MemberId;
use frame_support::{derive_impl, traits::EnsureOrigin};
use sp_runtime::{traits::IdentityLookup, BuildStorage};

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        Federation: pallet_federation,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
    type AccountId = u64;
    type Lookup = IdentityLookup<Self::AccountId>;
}

/// 測試用理事會來源:`Signed(account)` 依其數值映射到一個 `MemberId`
/// (= `CountryId`,兩個 ASCII 位元組)。
///
/// Test-only council origin: a `Signed(account)` maps its numeric id onto a
/// [`MemberId`] (= `CountryId`, two ASCII bytes), e.g. account `1` -> `b"A1"`.
pub struct EnsureCouncilMember;
impl EnsureOrigin<RuntimeOrigin> for EnsureCouncilMember {
    type Success = MemberId;

    fn try_origin(o: RuntimeOrigin) -> Result<Self::Success, RuntimeOrigin> {
        match o.clone().into() {
            Ok(frame_system::RawOrigin::Signed(who)) => Ok(account_to_member(who)),
            _ => Err(o),
        }
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn try_successful_origin() -> Result<RuntimeOrigin, ()> {
        Ok(frame_system::RawOrigin::Signed(1).into())
    }
}

/// 將測試帳號數值映射為一個固定的 `MemberId`(`CountryId`,§11.1 一國一席)。
///
/// Map a test account id onto a deterministic [`MemberId`] (`CountryId`,
/// §11.1 one seat per nation).
pub fn account_to_member(who: u64) -> MemberId {
    let members: [[u8; 2]; 4] = [*b"TW", *b"JP", *b"US", *b"EU"];
    members[(who as usize) % members.len()]
}

frame_support::parameter_types! {
    /// 測試用時間鎖長度(區塊數)/ Test-only timelock length (blocks).
    pub const TestTimelock: u64 = 10;
}

impl pallet_federation::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type CouncilMember = EnsureCouncilMember;
    type TimelockFor = TestTimelock;
    type WeightInfo = ();
}

/// 建立測試用初始狀態 / Build genesis storage for the mock runtime.
///
/// 區塊高度從 1 開始,以確保 `frame_system` 會記錄事件
/// (genesis 區塊 0 不記錄事件)。
///
/// Block number starts at 1 so that `frame_system` records events
/// (events are not recorded at the genesis block 0).
pub fn new_test_ext() -> sp_io::TestExternalities {
    let t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();
    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| System::set_block_number(1));
    ext
}
