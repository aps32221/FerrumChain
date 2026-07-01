//! # Ferrum 鐵鏈 — Runtime (主權執行時 / sovereign runtime)
//!
//! 以 `construct_runtime!` 組合 FRAME 系統 pallet 與 Ferrum 六大模組
//! (identity / credential / tax / treasury / federation / interop),於
//! **Aura + GRANDPA(PoSA)** 共識之上執行(白皮書 §03/§04/§07)。
//!
//! Composes the FRAME system pallets and the six Ferrum modules
//! (identity / credential / tax / treasury / federation / interop) via
//! `construct_runtime!`, running on **Aura + GRANDPA (PoSA)** consensus
//! (whitepaper §03/§04/§07).
//!
//! ## 跨模組接線 / Cross-module wiring
//! - `pallet-tax::Config::Treasury` 綁定到 [`TaxTreasuryAdapter`],它將
//!   `pallet_tax::TreasurySettle` 轉接到 `pallet_treasury_fer::Pallet`(見下方
//!   reconciliation 註解)。
//! - `pallet-federation::Config::CouncilMember` 由 [`EnsureCouncilMember`]
//!   將受認證理事帳戶映射為 `MemberId`(一國一席,§11.1)。
//! - `pallet-interop` 的 `FederationOrigin` / `RelayerOrigin` 由治理(Root)
//!   與簽署來源提供(§08)。

#![cfg_attr(not(feature = "std"), no_std)]
// `construct_runtime!` 會產生大量遞迴展開 / generated recursion from macros.
#![recursion_limit = "256"]

// Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

pub mod consensus;

extern crate alloc;
use alloc::vec::Vec;

use ferrum_primitives::{
    AccountId, Balance, BlockNumber, CountryId, Hash, MemberId, Moment, Nonce, FER, MAX_TAG_LEN,
    SLOT_DURATION_MS,
};
#[cfg(feature = "runtime-benchmarks")]
use codec::Decode;
use frame_support::{
    construct_runtime, derive_impl,
    genesis_builder_helper::{build_state, get_preset},
    parameter_types,
    traits::{ConstU32, ConstU64, ConstU8, EnsureOrigin},
    weights::{
        constants::{
            BlockExecutionWeight, ExtrinsicBaseWeight, RocksDbWeight, WEIGHT_REF_TIME_PER_SECOND,
        },
        IdentityFee, Weight,
    },
    BoundedVec,
};
use frame_system::{
    limits::{BlockLength, BlockWeights},
    EnsureRoot, EnsureSigned,
};
use pallet_grandpa::AuthorityId as GrandpaId;
use pallet_transaction_payment::{ConstFeeMultiplier, FungibleAdapter, Multiplier};
use sp_api::impl_runtime_apis;
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_core::{crypto::KeyTypeId, OpaqueMetadata};
use sp_runtime::{
    create_runtime_str, generic, impl_opaque_keys,
    traits::{AccountIdLookup, BlakeTwo256, Block as BlockT, NumberFor, One},
    transaction_validity::{TransactionSource, TransactionValidity},
    ApplyExtrinsicResult, MultiSignature,
};
#[cfg(feature = "std")]
use sp_version::NativeVersion;
use sp_version::RuntimeVersion;

/// Signature type used by the chain (re-exported alias for clarity).
pub type Signature = MultiSignature;

/// Opaque types used by the node service (block import, networking).
pub mod opaque {
    use super::*;
    use sp_runtime::generic;

    /// Opaque block header type.
    pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
    /// Opaque block type.
    pub type Block = generic::Block<Header, sp_runtime::OpaqueExtrinsic>;
    /// Opaque block id type.
    pub type BlockId = generic::BlockId<Block>;

    impl_opaque_keys! {
        /// Session keys: Aura authoring + GRANDPA finality (PoSA, §07).
        pub struct SessionKeys {
            pub aura: Aura,
            pub grandpa: Grandpa,
        }
    }
}
pub use opaque::SessionKeys;

// ============================================================================
// Runtime version (§03) — bump `spec_version` on every forkless upgrade.
// ============================================================================

#[sp_version::runtime_version]
pub const VERSION: RuntimeVersion = RuntimeVersion {
    spec_name: create_runtime_str!("ferrum"),
    impl_name: create_runtime_str!("ferrum"),
    authoring_version: 1,
    spec_version: 100,
    impl_version: 1,
    apis: RUNTIME_API_VERSIONS,
    transaction_version: 1,
    system_version: 1,
};

/// Native version used by the node executor.
#[cfg(feature = "std")]
pub fn native_version() -> NativeVersion {
    NativeVersion { runtime_version: VERSION, can_author_with: Default::default() }
}

// ============================================================================
// Common block/header/extrinsic aliases.
// ============================================================================

pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
pub type Block = generic::Block<Header, UncheckedExtrinsic>;
pub type SignedExtra = (
    frame_system::CheckNonZeroSender<Runtime>,
    frame_system::CheckSpecVersion<Runtime>,
    frame_system::CheckTxVersion<Runtime>,
    frame_system::CheckGenesis<Runtime>,
    frame_system::CheckEra<Runtime>,
    frame_system::CheckNonce<Runtime>,
    frame_system::CheckWeight<Runtime>,
    pallet_transaction_payment::ChargeTransactionPayment<Runtime>,
);
pub type UncheckedExtrinsic =
    generic::UncheckedExtrinsic<Address, RuntimeCall, Signature, SignedExtra>;
pub type Address = sp_runtime::MultiAddress<AccountId, ()>;
pub type SignedPayload = generic::SignedPayload<RuntimeCall, SignedExtra>;
pub type CheckedExtrinsic = generic::CheckedExtrinsic<AccountId, RuntimeCall, SignedExtra>;

/// Executive: dispatches incoming extrinsics to pallets (frame-executive, §03).
pub type Executive = frame_executive::Executive<
    Runtime,
    Block,
    frame_system::ChainContext<Runtime>,
    Runtime,
    AllPalletsWithSystem,
>;

// ============================================================================
// Consensus / block-resource constants.
// ============================================================================

/// 出塊時間 == 時槽長度(§07:3 秒)/ block time == slot length (3s, §07).
pub const MILLISECS_PER_BLOCK: Moment = SLOT_DURATION_MS;
pub const SLOT_DURATION: Moment = MILLISECS_PER_BLOCK;
pub const MINUTES: BlockNumber = 60_000 / (MILLISECS_PER_BLOCK as BlockNumber);
pub const HOURS: BlockNumber = MINUTES * 60;
pub const DAYS: BlockNumber = HOURS * 24;

/// 一秒可用的計算權重(2 秒區塊執行的 1/3 留給出塊/驗證開銷)。
const WEIGHT_PER_SECOND: Weight =
    Weight::from_parts(WEIGHT_REF_TIME_PER_SECOND.saturating_mul(2), u64::MAX);

parameter_types! {
    pub const BlockHashCount: BlockNumber = 2400;
    pub const Version: RuntimeVersion = VERSION;
    pub RuntimeBlockLength: BlockLength =
        BlockLength::max_with_normal_ratio(5 * 1024 * 1024, NORMAL_DISPATCH_RATIO);
    pub RuntimeBlockWeights: BlockWeights = BlockWeights::builder()
        .base_block(BlockExecutionWeight::get())
        .for_class(frame_support::dispatch::DispatchClass::all(), |weights| {
            weights.base_extrinsic = ExtrinsicBaseWeight::get();
        })
        .for_class(frame_support::dispatch::DispatchClass::Normal, |weights| {
            weights.max_total = Some(NORMAL_DISPATCH_RATIO * WEIGHT_PER_SECOND);
        })
        .for_class(frame_support::dispatch::DispatchClass::Operational, |weights| {
            weights.max_total = Some(WEIGHT_PER_SECOND);
            weights.reserved = Some(WEIGHT_PER_SECOND - NORMAL_DISPATCH_RATIO * WEIGHT_PER_SECOND);
        })
        .avg_block_initialization(sp_runtime::Perbill::from_percent(10))
        .build_or_panic();
    pub const SS58Prefix: u8 = 42;
}

const NORMAL_DISPATCH_RATIO: sp_runtime::Perbill = sp_runtime::Perbill::from_percent(75);

// ============================================================================
// frame_system::Config
// ============================================================================

#[derive_impl(frame_system::config_preludes::SolochainDefaultConfig)]
impl frame_system::Config for Runtime {
    type BaseCallFilter = frame_support::traits::Everything;
    type BlockWeights = RuntimeBlockWeights;
    type BlockLength = RuntimeBlockLength;
    type AccountId = AccountId;
    type Nonce = Nonce;
    type Hash = Hash;
    type Hashing = BlakeTwo256;
    type Block = Block;
    type Lookup = AccountIdLookup<AccountId, ()>;
    type RuntimeEvent = RuntimeEvent;
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    type RuntimeTask = RuntimeTask;
    type BlockHashCount = BlockHashCount;
    type DbWeight = RocksDbWeight;
    type Version = Version;
    type AccountData = pallet_balances::AccountData<Balance>;
    type SS58Prefix = SS58Prefix;
    type MaxConsumers = ConstU32<16>;
}

// ============================================================================
// pallet-timestamp (drives Aura slots, §07)
// ============================================================================

parameter_types! {
    pub const MinimumPeriod: Moment = SLOT_DURATION_MS / 2;
}

impl pallet_timestamp::Config for Runtime {
    type Moment = Moment;
    type OnTimestampSet = Aura;
    type MinimumPeriod = MinimumPeriod;
    type WeightInfo = ();
}

// ============================================================================
// pallet-aura — PoSA block authoring (§07)
// ============================================================================

impl pallet_aura::Config for Runtime {
    type AuthorityId = AuraId;
    type DisabledValidators = ();
    type MaxAuthorities = consensus::MaxAuthorities;
    type AllowMultipleBlocksPerSlot = frame_support::traits::ConstBool<false>;
    type SlotDuration = pallet_aura::MinimumPeriodTimesTwo<Runtime>;
}

// ============================================================================
// pallet-grandpa — PoSA BFT finality (§07)
// ============================================================================

impl pallet_grandpa::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type MaxAuthorities = consensus::MaxAuthorities;
    type MaxNominators = ConstU32<0>;
    type MaxSetIdSessionEntries = ConstU64<0>;
    type KeyOwnerProof = sp_core::Void;
    type EquivocationReportSystem = ();
}

// ============================================================================
// pallet-balances — FER network token ledger (§08)
// ============================================================================

parameter_types! {
    /// 存在性保證金(避免狀態垃圾)/ existential deposit guarding state bloat.
    pub const ExistentialDeposit: Balance = FER / 1_000;
    pub const MaxLocks: u32 = 50;
    pub const MaxReserves: u32 = 50;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Runtime {
    type MaxLocks = MaxLocks;
    type MaxReserves = MaxReserves;
    type ReserveIdentifier = [u8; 8];
    type Balance = Balance;
    type RuntimeEvent = RuntimeEvent;
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = pallet_balances::weights::SubstrateWeight<Runtime>;
    type FreezeIdentifier = ();
    type MaxFreezes = ();
    type RuntimeHoldReason = RuntimeHoldReason;
    type RuntimeFreezeReason = RuntimeFreezeReason;
}

// ============================================================================
// pallet-transaction-payment — fees in FER; citizen flows are fee-free (§08)
// ============================================================================

parameter_types! {
    pub FeeMultiplier: Multiplier = Multiplier::one();
}

impl pallet_transaction_payment::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    // 商業性重度用量繳基礎費(部分銷毀,§08);此處費用直接由帳戶扣除。
    // Heavy commercial usage pays base fees (partly burned, §08).
    type OnChargeTransaction = FungibleAdapter<Balances, ()>;
    type OperationalFeeMultiplier = ConstU8<5>;
    type WeightToFee = IdentityFee<Balance>;
    type LengthToFee = IdentityFee<Balance>;
    type FeeMultiplierUpdate = ConstFeeMultiplier<FeeMultiplier>;
    type WeightInfo = pallet_transaction_payment::weights::SubstrateWeight<Runtime>;
}

// ============================================================================
// pallet-sudo — bootstrap governance key (genesis council/issuer setup)
// ============================================================================

impl pallet_sudo::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type WeightInfo = pallet_sudo::weights::SubstrateWeight<Runtime>;
}

// ============================================================================
// pallet-identity-fer — DID registry (§05)
// ============================================================================

parameter_types! {
    /// 本地鏈標籤 `did:fer:tw`(§09)/ this chain's local tag `did:fer:tw`.
    pub LocalChainTag: BoundedVec<u8, ConstU32<MAX_TAG_LEN>> =
        BoundedVec::try_from(b"tw".to_vec()).expect("tag fits MAX_TAG_LEN; qed");
}

impl pallet_identity_fer::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    // 受認證簽發機構為任一簽署帳戶(實際認證由鏈上名冊 AccreditedIssuers 控管,§05)。
    // Issuer origin is any signed account; accreditation is gated by the
    // on-chain AccreditedIssuers roster, governed via `register_issuer`.
    type IssuerOrigin = EnsureSigned<AccountId>;
    type GovernanceOrigin = EnsureRoot<AccountId>;
    type LocalChainTag = LocalChainTag;
    type WeightInfo = ();
}

// ============================================================================
// pallet-credential — Verifiable Credentials (§05)
// ============================================================================

impl pallet_credential::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type IssuerOrigin = EnsureSigned<AccountId>;
    type WeightInfo = ();
}

// ============================================================================
// pallet-treasury-fer — FER pools / eTWD settlement (§08)
// ============================================================================

impl pallet_treasury_fer::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type GovernanceOrigin = EnsureRoot<AccountId>;
    type WeightInfo = ();
}

// ============================================================================
// Reconciliation adapter (#3): bridge the two distinct `TreasurySettle` traits.
// ----------------------------------------------------------------------------
// `pallet-tax` exports `pallet_tax::TreasurySettle` and `pallet-treasury-fer`
// exports its own `pallet_treasury_fer::TreasurySettle`. They are DIFFERENT
// traits, so `tax::Config::Treasury` cannot be bound directly to the treasury
// pallet. The orphan rule also forbids `impl pallet_tax::TreasurySettle for
// pallet_treasury_fer::Pallet` in this crate. We therefore define a local
// zero-sized adapter and implement `pallet_tax::TreasurySettle` for it,
// delegating to the treasury pallet's own `TreasurySettle` impl. No pallet
// source was changed; this is the sanctioned runtime-side wiring per SPEC §4's
// NOTE ("export it from pallet-treasury-fer and have pallet-tax depend on it").
// ============================================================================

/// 將 `pallet-tax` 的國庫結算需求轉接到 `pallet-treasury-fer`。
///
/// Adapter bridging `pallet_tax::TreasurySettle` to the treasury pallet's
/// `pallet_treasury_fer::TreasurySettle` impl.
pub struct TaxTreasuryAdapter;
impl pallet_tax::TreasurySettle<AccountId> for TaxTreasuryAdapter {
    fn settle_fiat(
        payer: &AccountId,
        receipt: ferrum_primitives::Hash32,
        amount: ferrum_primitives::FiatAmount,
    ) -> sp_runtime::DispatchResult {
        <Treasury as pallet_treasury_fer::TreasurySettle<AccountId>>::settle_fiat(
            payer, receipt, amount,
        )
    }
}

// ============================================================================
// pallet-tax — tax administration (§06)
// ============================================================================

impl pallet_tax::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Treasury = TaxTreasuryAdapter;
    // 已結算 VAT 稅收導入開獎系統的稅務等比率獎池(§06/§9)。
    // Settled VAT revenue feeds the lottery's tax-proportional pool (§06/§9).
    type RevenueHook = TaxRevenueAdapter;
    // 授權稽核員為治理(Root)指定 / auditors authorized via Root governance.
    type AuditorOrigin = EnsureRoot<AccountId>;
    type GovernanceOrigin = EnsureRoot<AccountId>;
    type WeightInfo = ();
}

// ============================================================================
// pallet-federation — treaty council, dual-majority, XSU basket (§11)
// ============================================================================

/// 將「受認證理事帳戶」映射為其 `MemberId`(一國一席,§11.1)。
///
/// Maps an accredited council account (a signed origin) to its `MemberId`.
/// In production the account<->seat binding is governed; here we derive a
/// deterministic 2-byte country code from the account's first two bytes so a
/// genesis-seeded council account always resolves to a stable seat.
pub struct EnsureCouncilMember;
impl EnsureOrigin<RuntimeOrigin> for EnsureCouncilMember {
    type Success = MemberId;

    fn try_origin(o: RuntimeOrigin) -> Result<Self::Success, RuntimeOrigin> {
        match o.clone().into() {
            Ok(frame_system::RawOrigin::Signed(who)) => {
                let bytes: &[u8] = who.as_ref();
                let member: CountryId = [bytes[0], bytes[1]];
                Ok(member)
            }
            _ => Err(o),
        }
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn try_successful_origin() -> Result<RuntimeOrigin, ()> {
        use sp_runtime::traits::TrailingZeroInput;
        let who = AccountId::decode(&mut TrailingZeroInput::zeroes())
            .map_err(|_| ())?;
        Ok(frame_system::RawOrigin::Signed(who).into())
    }
}

parameter_types! {
    /// 預設時間鎖長度(§11.2:參數調整 7 天;以區塊數表示)。
    /// Default timelock length (§11.2 parameter domain = 7 days, in blocks).
    pub const FederationTimelock: BlockNumber = 7 * DAYS;
}

impl pallet_federation::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type CouncilMember = EnsureCouncilMember;
    type TimelockFor = FederationTimelock;
    type WeightInfo = ();
}

// ============================================================================
// pallet-interop — cross-border bridge & clearing (§09–§10)
// ============================================================================

impl pallet_interop::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    // 聯邦治理來源(條約理事會)由 Root 代表 / federation origin via Root.
    type FederationOrigin = EnsureRoot<AccountId>;
    // 中繼者為任一簽署帳戶(實際名單由聯邦核准,§11.1)。
    // Relayers are signed accounts (approved set governed by federation).
    type RelayerOrigin = EnsureSigned<AccountId>;
    // 通用 DID 解析器透過身分 pallet 解析本鏈 DID(§09)。
    // The universal DID resolver resolves local DIDs via the identity pallet (§09).
    type DidRegistry = Identity;
    // 與身分 pallet 共用本地鏈標籤 `did:fer:tw`(§09)。
    // Shares the identity pallet's local chain tag `did:fer:tw` (§09).
    type LocalChainTag = LocalChainTag;
    type WeightInfo = ();
}

// ============================================================================
// pallet-lottery — e-invoice lottery, tax-proportional eTWD prizes (§06)
// ----------------------------------------------------------------------------
// Loosely coupled: the lottery composes pallet-tax (invoice anchors) and
// pallet-treasury-fer (attested eTWD reserve + PII-free prize receipts) through
// adapter structs that call each pallet's public read/notify API. All glue lives
// here in the runtime — the pallets stay independent (SPEC §9).
// ============================================================================

/// 將 `pallet-tax` 的發票錨定唯讀介面轉接給開獎系統(§06 票券資格)。
///
/// Adapts `pallet-tax`'s invoice-anchor read API to the lottery's
/// `InvoiceRegistry` (ticket eligibility + period windowing by block height).
pub struct LotteryTaxAdapter;
impl pallet_lottery::InvoiceRegistry for LotteryTaxAdapter {
    fn invoice_kind(invoice_hash: &ferrum_primitives::Hash32) -> Option<ferrum_primitives::TaxKind> {
        pallet_tax::Pallet::<Runtime>::invoice_kind(invoice_hash)
    }
    fn anchored_block(invoice_hash: &ferrum_primitives::Hash32) -> Option<ferrum_primitives::BlockNumber> {
        use sp_runtime::SaturatedConversion;
        pallet_tax::Pallet::<Runtime>::anchored_block(invoice_hash).map(|b| b.saturated_into())
    }
    fn is_anchored(invoice_hash: &ferrum_primitives::Hash32) -> bool {
        pallet_tax::Pallet::<Runtime>::is_anchored(invoice_hash)
    }
}

/// 將得獎給付轉接給 `pallet-treasury-fer` 的去識別化 eTWD 收據(價值走 CBDC 軌道)。
///
/// Adapts prize payout to the treasury's PII-free eTWD receipt recorder.
pub struct LotteryTreasuryAdapter;
impl pallet_lottery::TreasuryPayout<AccountId> for LotteryTreasuryAdapter {
    fn credit_fiat(
        beneficiary: &AccountId,
        receipt_key: ferrum_primitives::Hash32,
        amount: ferrum_primitives::FiatAmount,
    ) -> sp_runtime::DispatchResult {
        pallet_treasury_fer::Pallet::<Runtime>::credit_prize(beneficiary, receipt_key, amount)
    }
}

/// 將獎池準備封頂/扣減/回流轉接給央行認證之 eTWD 準備餘額。
///
/// Adapts the prize-pool clamp/debit/recycle to the central-bank-attested eTWD
/// reserve in `pallet-treasury-fer`.
pub struct LotteryReserveAdapter;
impl pallet_lottery::AttestedReserve for LotteryReserveAdapter {
    fn attested_balance() -> ferrum_primitives::FiatAmount {
        pallet_treasury_fer::Pallet::<Runtime>::attested_etwd()
            .unwrap_or(ferrum_primitives::FiatAmount { currency: *b"TWD", minor_units: 0 })
    }
    fn try_debit(amount: ferrum_primitives::FiatAmount) -> sp_runtime::DispatchResult {
        pallet_treasury_fer::Pallet::<Runtime>::try_debit_etwd(amount)
    }
    fn credit(amount: ferrum_primitives::FiatAmount) {
        pallet_treasury_fer::Pallet::<Runtime>::credit_etwd(amount)
    }
}

/// 將 `pallet-tax` 的已結算稅收回呼導入開獎系統的稅務等比率獎池(僅 `ValueAdded`)。
///
/// Feeds `pallet-tax`'s settled-revenue hook into the lottery's
/// tax-proportional pool — only for `ValueAdded` (VAT) settlements (§9).
pub struct TaxRevenueAdapter;
impl pallet_tax::RevenueSink for TaxRevenueAdapter {
    fn note_settled(kind: ferrum_primitives::TaxKind, amount: ferrum_primitives::FiatAmount) {
        if kind == ferrum_primitives::TaxKind::ValueAdded {
            pallet_lottery::Pallet::<Runtime>::note_settled_revenue(amount);
        }
    }
}

parameter_types! {
    /// 開獎獎金幣別 = eTWD。/ Prize currency = eTWD.
    pub const LotteryPrizeCurrency: ferrum_primitives::FiatCurrency = *b"TWD";
    /// 憲制級資金比率上限 2%。/ Constitutional funding-ratio ceiling 2%.
    pub const LotteryMaxRatioPpm: u32 = 20_000;
    /// 有效抽獎所需揭示法定數。/ Quorum of reveals for a valid draw.
    pub const LotteryMinReveals: u32 = 3;
    /// 抽獎承諾保證金。/ Commit bond.
    pub const LotteryCommitDeposit: Balance = 250_000 * FER;
    /// 年齡述詞門檻(資格電路)。/ Age-predicate threshold (eligibility circuit).
    pub const LotteryAgeThreshold: u32 = 18;
}

impl pallet_lottery::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Tax = LotteryTaxAdapter;
    type AgeThreshold = LotteryAgeThreshold;
    type PrizeTreasury = LotteryTreasuryAdapter;
    type EtwdReserve = LotteryReserveAdapter;
    // 治理 / 緊急 / 商家註冊皆由治理(Root)代表 / governed via Root in this build.
    type GovernanceOrigin = EnsureRoot<AccountId>;
    type EmergencyOrigin = EnsureRoot<AccountId>;
    type RegistrarOrigin = EnsureRoot<AccountId>;
    type PrizeCurrency = LotteryPrizeCurrency;
    type MaxRatioPpm = LotteryMaxRatioPpm;
    type MinReveals = LotteryMinReveals;
    type CommitDeposit = LotteryCommitDeposit;
    type Currency = Balances;
    type WeightInfo = ();
}

// ============================================================================
// construct_runtime! — 組合所有 pallet / compose all pallets
// ============================================================================

construct_runtime!(
    pub enum Runtime {
        // FRAME system + consensus.
        System: frame_system = 0,
        Timestamp: pallet_timestamp = 1,
        Aura: pallet_aura = 2,
        Grandpa: pallet_grandpa = 3,

        // Economic layer.
        Balances: pallet_balances = 4,
        TransactionPayment: pallet_transaction_payment = 5,
        Sudo: pallet_sudo = 6,

        // Ferrum sovereign modules (§05/§06/§08/§09/§11).
        Identity: pallet_identity_fer = 10,
        Credential: pallet_credential = 11,
        Tax: pallet_tax = 12,
        Treasury: pallet_treasury_fer = 13,
        Federation: pallet_federation = 14,
        Interop: pallet_interop = 15,
        Lottery: pallet_lottery = 16,
    }
);

// ============================================================================
// Benchmarking registry (gated).
// ============================================================================

#[cfg(feature = "runtime-benchmarks")]
mod benches {
    frame_benchmarking::define_benchmarks!(
        [frame_system, SystemBench::<Runtime>]
        [pallet_balances, Balances]
        [pallet_timestamp, Timestamp]
        [pallet_identity_fer, Identity]
        [pallet_credential, Credential]
        [pallet_tax, Tax]
        [pallet_treasury_fer, Treasury]
        [pallet_federation, Federation]
        [pallet_interop, Interop]
    );
}

// ============================================================================
// Runtime API implementations (§03 — what the node calls into the WASM runtime).
// ============================================================================

impl_runtime_apis! {
    impl sp_api::Core<Block> for Runtime {
        fn version() -> RuntimeVersion {
            VERSION
        }

        fn execute_block(block: Block) {
            Executive::execute_block(block);
        }

        fn initialize_block(header: &<Block as BlockT>::Header) -> sp_runtime::ExtrinsicInclusionMode {
            Executive::initialize_block(header)
        }
    }

    impl sp_api::Metadata<Block> for Runtime {
        fn metadata() -> OpaqueMetadata {
            OpaqueMetadata::new(Runtime::metadata().into())
        }

        fn metadata_at_version(version: u32) -> Option<OpaqueMetadata> {
            Runtime::metadata_at_version(version)
        }

        fn metadata_versions() -> Vec<u32> {
            Runtime::metadata_versions()
        }
    }

    impl sp_block_builder::BlockBuilder<Block> for Runtime {
        fn apply_extrinsic(extrinsic: <Block as BlockT>::Extrinsic) -> ApplyExtrinsicResult {
            Executive::apply_extrinsic(extrinsic)
        }

        fn finalize_block() -> <Block as BlockT>::Header {
            Executive::finalize_block()
        }

        fn inherent_extrinsics(data: sp_inherents::InherentData) -> Vec<<Block as BlockT>::Extrinsic> {
            data.create_extrinsics()
        }

        fn check_inherents(
            block: Block,
            data: sp_inherents::InherentData,
        ) -> sp_inherents::CheckInherentsResult {
            data.check_extrinsics(&block)
        }
    }

    impl sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block> for Runtime {
        fn validate_transaction(
            source: TransactionSource,
            tx: <Block as BlockT>::Extrinsic,
            block_hash: <Block as BlockT>::Hash,
        ) -> TransactionValidity {
            Executive::validate_transaction(source, tx, block_hash)
        }
    }

    impl sp_offchain::OffchainWorkerApi<Block> for Runtime {
        fn offchain_worker(header: &<Block as BlockT>::Header) {
            Executive::offchain_worker(header)
        }
    }

    impl sp_consensus_aura::AuraApi<Block, AuraId> for Runtime {
        fn slot_duration() -> sp_consensus_aura::SlotDuration {
            sp_consensus_aura::SlotDuration::from_millis(Aura::slot_duration())
        }

        fn authorities() -> Vec<AuraId> {
            pallet_aura::Authorities::<Runtime>::get().into_inner()
        }
    }

    impl sp_session::SessionKeys<Block> for Runtime {
        fn generate_session_keys(seed: Option<Vec<u8>>) -> Vec<u8> {
            opaque::SessionKeys::generate(seed)
        }

        fn decode_session_keys(encoded: Vec<u8>) -> Option<Vec<(Vec<u8>, KeyTypeId)>> {
            opaque::SessionKeys::decode_into_raw_public_keys(&encoded)
        }
    }

    impl sp_consensus_grandpa::GrandpaApi<Block> for Runtime {
        fn grandpa_authorities() -> sp_consensus_grandpa::AuthorityList {
            Grandpa::grandpa_authorities()
        }

        fn current_set_id() -> sp_consensus_grandpa::SetId {
            Grandpa::current_set_id()
        }

        fn submit_report_equivocation_unsigned_extrinsic(
            _equivocation_proof: sp_consensus_grandpa::EquivocationProof<
                <Block as BlockT>::Hash,
                NumberFor<Block>,
            >,
            _key_owner_proof: sp_consensus_grandpa::OpaqueKeyOwnershipProof,
        ) -> Option<()> {
            None
        }

        fn generate_key_ownership_proof(
            _set_id: sp_consensus_grandpa::SetId,
            _authority_id: GrandpaId,
        ) -> Option<sp_consensus_grandpa::OpaqueKeyOwnershipProof> {
            None
        }
    }

    impl frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Nonce> for Runtime {
        fn account_nonce(account: AccountId) -> Nonce {
            System::account_nonce(account)
        }
    }

    impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<Block, Balance> for Runtime {
        fn query_info(
            uxt: <Block as BlockT>::Extrinsic,
            len: u32,
        ) -> pallet_transaction_payment_rpc_runtime_api::RuntimeDispatchInfo<Balance> {
            TransactionPayment::query_info(uxt, len)
        }
        fn query_fee_details(
            uxt: <Block as BlockT>::Extrinsic,
            len: u32,
        ) -> pallet_transaction_payment::FeeDetails<Balance> {
            TransactionPayment::query_fee_details(uxt, len)
        }
        fn query_weight_to_fee(weight: Weight) -> Balance {
            TransactionPayment::weight_to_fee(weight)
        }
        fn query_length_to_fee(length: u32) -> Balance {
            TransactionPayment::length_to_fee(length)
        }
    }

    impl sp_genesis_builder::GenesisBuilder<Block> for Runtime {
        fn build_state(config: Vec<u8>) -> sp_genesis_builder::Result {
            build_state::<RuntimeGenesisConfig>(config)
        }

        fn get_preset(id: &Option<sp_genesis_builder::PresetId>) -> Option<Vec<u8>> {
            get_preset::<RuntimeGenesisConfig>(id, |_| None)
        }

        fn preset_names() -> Vec<sp_genesis_builder::PresetId> {
            Default::default()
        }
    }

    #[cfg(feature = "runtime-benchmarks")]
    impl frame_benchmarking::Benchmark<Block> for Runtime {
        fn benchmark_metadata(extra: bool) -> (
            Vec<frame_benchmarking::BenchmarkList>,
            Vec<frame_support::traits::StorageInfo>,
        ) {
            use frame_benchmarking::{baseline, BenchmarkList};
            use frame_support::traits::StorageInfoTrait;
            use frame_system_benchmarking::Pallet as SystemBench;
            use baseline::Pallet as BaselineBench;

            let mut list = Vec::<BenchmarkList>::new();
            list_benchmarks!(list, extra);
            let storage_info = AllPalletsWithSystem::storage_info();
            (list, storage_info)
        }

        fn dispatch_benchmark(
            config: frame_benchmarking::BenchmarkConfig,
        ) -> Result<Vec<frame_benchmarking::BenchmarkBatch>, alloc::string::String> {
            use frame_benchmarking::{baseline, BenchmarkBatch};
            use frame_support::traits::TrackedStorageKey;
            use frame_system_benchmarking::Pallet as SystemBench;
            use baseline::Pallet as BaselineBench;

            impl frame_system_benchmarking::Config for Runtime {}
            impl baseline::Config for Runtime {}

            let whitelist: Vec<TrackedStorageKey> = Vec::new();
            let mut batches = Vec::<BenchmarkBatch>::new();
            let params = (&config, &whitelist);
            add_benchmarks!(params, batches);
            Ok(batches)
        }
    }

    #[cfg(feature = "try-runtime")]
    impl frame_try_runtime::TryRuntime<Block> for Runtime {
        fn on_runtime_upgrade(checks: frame_try_runtime::UpgradeCheckSelect) -> (Weight, Weight) {
            let weight = Executive::try_runtime_upgrade(checks).unwrap();
            (weight, RuntimeBlockWeights::get().max_block)
        }

        fn execute_block(
            block: Block,
            state_root_check: bool,
            signature_check: bool,
            select: frame_try_runtime::TryStateSelect,
        ) -> Weight {
            Executive::try_execute_block(block, state_root_check, signature_check, select).unwrap()
        }
    }
}

