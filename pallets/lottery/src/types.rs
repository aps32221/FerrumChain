//! Canonical types for `pallet-lottery` (whitepaper §06 e-invoice lottery).
//! All are PII-free: only commitments, hashes, ids and fiat accounting amounts.

use codec::{Decode, Encode, MaxEncodedLen};
use ferrum_primitives::{BlockNumber, Commitment, FiatAmount, Hash32, TaxKind};
use frame_support::{pallet_prelude::ConstU32, BoundedVec};
use scale_info::TypeInfo;
use sp_runtime::RuntimeDebug;

/// One draw per period; `DrawId == PeriodId`.
pub type DrawId = u64;
pub type PeriodId = u64;

/// A registered entry. Ticket identity is the `invoice_hash` storage key — this
/// struct holds only binding commitments. No PII, no buyer DID on-chain.
#[derive(Clone, Eq, PartialEq, Encode, Decode, MaxEncodedLen, TypeInfo, RuntimeDebug)]
pub struct LotteryTicket {
    pub draw: DrawId,
    /// `owner_commitment = BLAKE2b(owner_did ‖ invoice_hash ‖ owner_secret)`.
    pub owner_commitment: Commitment,
    pub registered_at: BlockNumber,
}

/// Single draw lifecycle. A "ticketing view" is derived via `Pallet::phase`.
#[derive(Clone, Copy, Eq, PartialEq, Encode, Decode, MaxEncodedLen, TypeInfo, RuntimeDebug)]
pub enum DrawState {
    Pending,
    Open,
    Drawing,
    Drawn,
    Settled,
    Cancelled,
}

/// One prize tier: a fixed pool share, a winner count and a per-winner cap.
#[derive(Clone, Eq, PartialEq, Encode, Decode, MaxEncodedLen, TypeInfo, RuntimeDebug)]
pub struct PrizeTier {
    pub tier_id: u8,
    /// Share of the pool in parts-per-million; `Σ share_ppm == 1_000_000`.
    pub share_ppm: u32,
    pub winners: u32,
    pub unit_cap: FiatAmount,
}

/// The governed draw template; snapshotted immutably into each [`DrawRecord`].
#[derive(Clone, Eq, PartialEq, Encode, Decode, MaxEncodedLen, TypeInfo, RuntimeDebug)]
pub struct DrawConfig {
    pub period_blocks: BlockNumber,
    pub eligible_kinds: BoundedVec<TaxKind, ConstU32<8>>,
    /// Funding ratio r (ppm): pool = r × authenticated period tax revenue.
    pub tax_ratio_ppm: u32,
    /// Max pool as a share (ppm) of the attested eTWD reserve.
    pub reserve_cap_ppm: u32,
    pub tiers: BoundedVec<PrizeTier, ConstU32<16>>,
    pub allow_foreign: bool,
    pub commit_deadline: BlockNumber,
    /// MUST be `< finalize_block` (enforced on `reveal`).
    pub reveal_deadline: BlockNumber,
    /// The GRANDPA-finalized entropy-anchor block; `> reveal_deadline`.
    pub finalize_block: BlockNumber,
    pub claim_window: BlockNumber,
}

/// Per-draw immutable snapshot plus mutable lifecycle state.
#[derive(Clone, Eq, PartialEq, Encode, Decode, MaxEncodedLen, TypeInfo, RuntimeDebug)]
pub struct DrawRecord {
    /// Snapshotted at open — immutable thereafter.
    pub config: DrawConfig,
    pub period_start_block: BlockNumber,
    pub period_end_block: BlockNumber,
    /// Frozen `PeriodTaxRevenue` at the period-end transition.
    pub revenue_snapshot: Option<FiatAmount>,
    pub state: DrawState,
}

/// A validator's commit–reveal entry for the draw entropy.
#[derive(Clone, Eq, PartialEq, Encode, Decode, MaxEncodedLen, TypeInfo, RuntimeDebug)]
pub struct Commit {
    pub commitment: Hash32,
    pub revealed: bool,
}
