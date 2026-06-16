//! # Ferrum 鐵鏈 — `runtime/src/consensus.rs`
//! ## PoSA: Aura authoring + GRANDPA finality; governed validator set (§07)
//!
//! 共識:**質押權威證明 (PoSA)** —— 出塊由 **Aura** 依時槽輪值受認證驗證者,
//! 最終性由 **GRANDPA** 提供 BFT 保證。驗證者集合(權威集)由**鏈上治理**控制,
//! 投票權不依質押量加權(合格驗證者近乎等權)。每位驗證者須質押 FER 作罰沒
//! 保證金:雙簽全沒收、離線輕罰(§07)。
//!
//! Consensus: **Proof of Staked Authority (PoSA)** — block authoring is done by
//! accredited validators via **Aura** slot rotation, and **GRANDPA** provides
//! BFT finality. The authority (validator) set is **governed on-chain**; voting
//! power is not stake-weighted (eligible validators are near-equal). Each
//! validator posts a FER slashing bond: equivocation is fully seized, offline is
//! lightly slashed (§07).
//!
//! This module reproduces the whitepaper §07 consensus excerpt verbatim and
//! exposes the consensus `parameter_types!` + Aura/GRANDPA wiring helpers the
//! runtime composes.

use ferrum_primitives::{Balance, FER};
pub use ferrum_primitives::{MAX_AUTHORITIES, SLOT_DURATION_MS};
use frame_support::parameter_types;
use sp_runtime::Perbill;

// ============================================================================
// 白皮書 §07 共識常數節錄(逐字)/ Whitepaper §07 consensus excerpt (verbatim)
// ----------------------------------------------------------------------------
// // PoSA: Aura authoring + GRANDPA finality; validator set is governed
// parameter_types! {
//     pub const SlotDuration: u64 = 3_000;            // one 3-second authoring slot
//     pub const MaxAuthorities: u32 = 100;            // cap on accredited institutional nodes
//     pub const MinValidatorBond: Balance = 250_000 * FER;
//     pub const EquivocationSlash: Perbill = Perbill::from_percent(100); // equivocation: full seizure
//     pub const OfflineSlash: Perbill = Perbill::from_percent(1);
// }
// ============================================================================

// PoSA: Aura authoring + GRANDPA finality; validator set is governed
parameter_types! {
    /// 出塊時槽長度:3 秒(== [`SLOT_DURATION_MS`]) / one 3-second authoring slot.
    pub const SlotDuration: u64 = SLOT_DURATION_MS;
    /// 受認證機構節點上限(== [`MAX_AUTHORITIES`]) / cap on accredited institutional nodes.
    pub const MaxAuthorities: u32 = MAX_AUTHORITIES;
    /// 驗證者最低質押保證金 / minimum validator staking bond.
    pub const MinValidatorBond: Balance = 250_000 * FER;
    /// 雙簽罰沒比率:全沒收 / equivocation slash: full seizure.
    pub const EquivocationSlash: Perbill = Perbill::from_percent(100);
    /// 離線罰沒比率:1% / offline slash: 1%.
    pub const OfflineSlash: Perbill = Perbill::from_percent(1);
}

parameter_types! {
    /// GRANDPA 可緩存的最大已撤銷授權集合數量。
    /// Max number of authority-set changes GRANDPA keeps for finality proofs.
    pub const MaxSetIdSessionEntries: u32 = 0;
    /// 等同 `MaxAuthorities`,供 GRANDPA `MaxAuthorities` 關聯型別使用。
    pub const MaxNominators: u32 = 0;
}
