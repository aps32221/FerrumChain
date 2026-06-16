//! # pallet-treasury-fer — 國庫 / 雙資產代幣模型 (whitepaper §08)
//! # pallet-treasury-fer — Treasury / Dual-asset Token Model (whitepaper §08)
//!
//! 本模組管理 **FER**（網路代幣，用於驗證者質押保證金、治理投票與資源計費，
//! 非投機性資產）的創世配置池、治理控制的低通膨發行（約年增 3%）、
//! EIP-1559 式基礎費銷毀、補貼基金（讓公民身分驗證與基本報稅免費），
//! 並接收來自 `pallet-tax` 的 **eTWD**（法幣計價）稅務結算收據。
//!
//! This pallet manages the genesis allocation pools for **FER** (the
//! non-speculative network token used for validator staking bonds,
//! governance votes and rare-case resource metering), governed low-inflation
//! issuance (~3%/yr), EIP-1559-style base-fee burn, and the subsidy fund that
//! makes citizen identity checks & basic filing fee-free (§08). It also
//! receives **eTWD** (fiat-denominated) tax-settlement receipts from
//! `pallet-tax` via the [`TreasurySettle`] trait.
//!
//! ## 隱私不變式 / Privacy invariant
//! 本模組不儲存任何個人識別資料（PII）。`EtwdReceipts` 僅以
//! [`ferrum_primitives::Hash32`] 承諾鍵入金額，永不含明細。
//!
//! This pallet stores no PII. [`EtwdReceipts`] is keyed by a
//! [`ferrum_primitives::Hash32`] commitment to the off-chain receipt detail —
//! never the detail itself.
//!
//! ## 創世配置池（§08 配置圖）/ Genesis allocation pools (§08 allocation chart)
//! | Pool id | 名稱 / Name                                         | 權重 / Weight |
//! |---------|------------------------------------------------------|--------------|
//! | 0       | 驗證者質押與安全池 / Validator staking & security pool | 30%          |
//! | 1       | 主權國庫儲備 / Sovereign treasury reserve              | 25%          |
//! | 2       | 公共服務補貼基金 / Public-service subsidy fund         | 20%          |
//! | 3       | 協定開發與維運 / Protocol dev & maintenance            | 15%          |
//! | 4       | 生態整合補助 / Ecosystem & integration grants          | 10%          |
//!
//! ## 核心機制 / Core mechanisms
//! - **發行政策（治理控制的低度通膨）** — `mint`: governance mints new FER into a
//!   named pool, modeling the ~3%/yr issuance for validator rewards (§08).
//! - **基礎費銷毀（EIP-1559 式）** — `burn`: burns FER from the caller's balance
//!   and tallies [`TotalBurned`], offsetting inflation.
//! - **補貼（身分驗證 / 基本報稅免費）** — `subsidize`: pays out from the subsidy
//!   pool (pool id 2) to cover a citizen's fee-free service.
//! - **稅務結算收據（eTWD）** — `record_settlement`: anchors a fiat-denominated
//!   settlement receipt commitment from `pallet-tax`, implementing
//!   [`TreasurySettle`].
#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(any(test, feature = "runtime-benchmarks"))]
pub mod mock;
#[cfg(test)]
mod tests;

pub mod weights;
pub use weights::WeightInfo;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[frame_support::pallet]
pub mod pallet {
    use super::WeightInfo;
    use ferrum_primitives::{Balance, FiatAmount, Hash32};
    use frame_support::{
        pallet_prelude::*,
        traits::{Currency, ExistenceRequirement, WithdrawReasons},
    };
    use frame_system::pallet_prelude::*;

    // ========================================================================
    // 池識別碼 / Pool identifiers (whitepaper §08 genesis allocation chart)
    // ========================================================================

    /// 驗證者質押與安全池 30% / Validator staking & security pool 30%.
    pub const POOL_STAKING_SECURITY: u8 = 0;
    /// 主權國庫儲備 25% / Sovereign treasury reserve 25%.
    pub const POOL_SOVEREIGN_RESERVE: u8 = 1;
    /// 公共服務補貼基金 20% / Public-service subsidy fund 20%.
    pub const POOL_SUBSIDY: u8 = 2;
    /// 協定開發與維運 15% / Protocol dev & maintenance 15%.
    pub const POOL_PROTOCOL_DEV: u8 = 3;
    /// 生態整合補助 10% / Ecosystem & integration grants 10%.
    pub const POOL_ECOSYSTEM: u8 = 4;

    /// 所有有效池識別碼，依 §08 配置圖順序排列。
    ///
    /// All valid pool ids, in the order they appear in the §08 allocation
    /// chart (30/25/20/15/10%).
    pub const ALL_POOLS: [u8; 5] = [
        POOL_STAKING_SECURITY,
        POOL_SOVEREIGN_RESERVE,
        POOL_SUBSIDY,
        POOL_PROTOCOL_DEV,
        POOL_ECOSYSTEM,
    ];

    /// 國庫結算介面 — 供 `pallet-tax` 消費（§06/§08 跨模組合約）。
    ///
    /// Treasury settlement interface consumed by `pallet-tax` (SPEC §06/§08
    /// cross-module contract). Implemented by [`Pallet`] below.
    pub trait TreasurySettle<AccountId> {
        /// 記錄一筆以法幣（eTWD）計價的稅務結算收據承諾。
        ///
        /// Record a fiat-denominated (eTWD) tax-settlement receipt commitment
        /// on behalf of `payer`.
        fn settle_fiat(payer: &AccountId, receipt: Hash32, amount: FiatAmount) -> DispatchResult;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// Config trait — the public surface the runtime wires (SPEC §08).
    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// 事件類型 / The overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// FER 代幣的貨幣實作（網路代幣，非投機性資產，§08）。
        ///
        /// The currency implementation for **FER**, the non-speculative
        /// network token (§08).
        type Currency: Currency<Self::AccountId, Balance = Balance>;

        /// 治理來源 — 控制發行（鑄造）與配置池管理。
        ///
        /// Governance origin — controls issuance (minting) and pool
        /// management (§08: "Issuance is treasury/governance-controlled").
        type GovernanceOrigin: EnsureOrigin<Self::RuntimeOrigin>;

        /// 權重資訊 / Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    // ========================================================================
    // Storage — 儲存項
    // ========================================================================

    /// 創世配置池餘額表：池識別碼 -> FER 餘額。
    ///
    /// 池 0=驗證者質押與安全(30%)、1=主權國庫儲備(25%)、2=公共服務補貼基金(20%)、
    /// 3=協定開發與維運(15%)、4=生態整合補助(10%)（§08 配置圖）。
    ///
    /// Genesis allocation pool balances keyed by pool id. Pool 0 = validator
    /// staking & security (30%), 1 = sovereign treasury reserve (25%),
    /// 2 = public-service subsidy fund (20%), 3 = protocol dev & maintenance
    /// (15%), 4 = ecosystem & integration grants (10%) — §08 allocation chart.
    #[pallet::storage]
    pub type Pools<T: Config> = StorageMap<_, Blake2_128Concat, u8, Balance, ValueQuery>;

    /// eTWD 稅務結算收據表：收據承諾雜湊 -> 法幣金額（來自 `pallet-tax`）。
    ///
    /// Tax-settlement receipt commitments keyed by [`Hash32`], mapping to the
    /// fiat amount settled (from `pallet-tax`, §06/§08). Detail stays
    /// off-chain — only the commitment and amount are anchored.
    #[pallet::storage]
    pub type EtwdReceipts<T: Config> = StorageMap<_, Blake2_128Concat, Hash32, FiatAmount>;

    /// 累計已銷毀的 FER 總量（EIP-1559 式基礎費銷毀，§08）。
    ///
    /// Running total of FER burned via the EIP-1559-style base-fee burn
    /// (§08), offsetting the ~3%/yr issuance.
    #[pallet::storage]
    pub type TotalBurned<T: Config> = StorageValue<_, Balance, ValueQuery>;

    // ========================================================================
    // Events — 事件
    // ========================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// 治理已鑄造 FER 進入指定池。`(pool, amount, new_balance)`。
        ///
        /// Governance minted FER into a pool. `(pool, amount, new_balance)`.
        Minted { pool: u8, amount: Balance, new_balance: Balance },
        /// 銷毀 FER（基礎費銷毀）。`(who, amount, total_burned)`。
        ///
        /// FER burned (base-fee burn). `(who, amount, total_burned)`.
        Burned { who: T::AccountId, amount: Balance, total_burned: Balance },
        /// 補貼基金支付一筆免費服務成本。`(pool, who, amount)`。
        ///
        /// Subsidy fund paid out for a fee-free service. `(pool, who, amount)`.
        Subsidized { pool: u8, who: T::AccountId, amount: Balance },
        /// 已記錄一筆 eTWD 稅務結算收據。`(payer, receipt, amount)`。
        ///
        /// A fiat (eTWD) tax-settlement receipt was recorded.
        /// `(payer, receipt, amount)`.
        SettlementRecorded { payer: T::AccountId, receipt: Hash32, amount: FiatAmount },
    }

    // ========================================================================
    // Errors — 錯誤
    // ========================================================================

    #[pallet::error]
    pub enum Error<T> {
        /// 未知的配置池識別碼（必須屬於 `ALL_POOLS`）。
        ///
        /// Unknown pool id (must be one of [`ALL_POOLS`]).
        UnknownPool,
        /// 該池餘額不足以支付請求金額。
        ///
        /// The pool balance is insufficient for the requested amount.
        InsufficientPoolBalance,
        /// 呼叫者帳戶餘額不足以銷毀請求金額。
        ///
        /// The caller's account balance is insufficient to burn the
        /// requested amount.
        InsufficientBalance,
        /// 該收據承諾已存在，禁止重放。
        ///
        /// A settlement receipt with this commitment already exists
        /// (replay protection).
        ReceiptAlreadyRecorded,
    }

    // ========================================================================
    // Calls — 可呼叫的外部函式
    // ========================================================================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// 治理鑄造 FER 進入指定配置池（§08：治理控制的低度通膨發行，約年增 3%）。
        ///
        /// Governance-only: mint `amount` of FER into allocation `pool`,
        /// modeling the governed low-inflation issuance (~3%/yr) used to
        /// fund validator rewards and the other §08 pools.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::mint())]
        pub fn mint(origin: OriginFor<T>, pool: u8, amount: Balance) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;
            ensure!(ALL_POOLS.contains(&pool), Error::<T>::UnknownPool);

            // 鑄造對應的可流通供給，使帳本與配置池餘額一致。
            // Mint the corresponding circulating supply so the issuable
            // total backs the pool's ledger balance.
            let _ = T::Currency::issue(amount);

            let new_balance = Pools::<T>::mutate(pool, |bal| {
                *bal = bal.saturating_add(amount);
                *bal
            });

            Self::deposit_event(Event::Minted { pool, amount, new_balance });
            Ok(())
        }

        /// 銷毀呼叫者帳戶中的 FER（EIP-1559 式基礎費銷毀，§08：
        /// 商業性重度用量的基礎費銷毀以對沖通膨）。
        ///
        /// Burn `amount` of FER from the caller's account balance
        /// (EIP-1559-style base-fee burn, §08), offsetting issuance.
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::burn())]
        pub fn burn(origin: OriginFor<T>, amount: Balance) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let _ = T::Currency::withdraw(
                &who,
                amount,
                WithdrawReasons::FEE,
                ExistenceRequirement::AllowDeath,
            )
            .map_err(|_| Error::<T>::InsufficientBalance)?;

            // 銷毀後對應供給亦從帳本移除。
            // The withdrawn supply is removed from the issuable total. The
            // returned imbalance is dropped on purpose: dropping a negative
            // imbalance from `burn` is what actually reduces total issuance.
            drop(T::Currency::burn(amount));

            let total_burned = TotalBurned::<T>::mutate(|t| {
                *t = t.saturating_add(amount);
                *t
            });

            Self::deposit_event(Event::Burned { who, amount, total_burned });
            Ok(())
        }

        /// 補貼基金支付一筆免費服務成本（§08：身分驗證與基本報稅免費，
        /// 成本由補貼基金（池 2）支付）。
        ///
        /// Pay `amount` of FER from the public-service subsidy fund
        /// (pool [`POOL_SUBSIDY`]) to `who`, covering a fee-free identity
        /// check or basic filing (§08).
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::subsidize())]
        pub fn subsidize(origin: OriginFor<T>, who: T::AccountId, amount: Balance) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;

            Pools::<T>::try_mutate(POOL_SUBSIDY, |bal| -> DispatchResult {
                ensure!(*bal >= amount, Error::<T>::InsufficientPoolBalance);
                *bal -= amount;
                Ok(())
            })?;

            let _ = T::Currency::deposit_creating(&who, amount);

            Self::deposit_event(Event::Subsidized { pool: POOL_SUBSIDY, who, amount });
            Ok(())
        }

        /// 記錄一筆來自 `pallet-tax` 的 eTWD 稅務結算收據承諾（§06/§08）。
        ///
        /// Record a fiat-denominated (eTWD) tax-settlement receipt
        /// commitment originating from `pallet-tax` (§06/§08). This is the
        /// extrinsic form of [`TreasurySettle::settle_fiat`].
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::record_settlement())]
        pub fn record_settlement(
            origin: OriginFor<T>,
            receipt: Hash32,
            amount: FiatAmount,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::do_record_settlement(&who, receipt, amount)
        }
    }

    // ========================================================================
    // Internal helpers — 內部輔助函式
    // ========================================================================

    impl<T: Config> Pallet<T> {
        /// 共用實作：記錄 eTWD 結算收據，供外部函式與 `TreasurySettle` 介面共用。
        ///
        /// Shared implementation recording an eTWD settlement receipt; used
        /// by both the `record_settlement` extrinsic and the
        /// [`TreasurySettle`] trait impl below.
        fn do_record_settlement(
            payer: &T::AccountId,
            receipt: Hash32,
            amount: FiatAmount,
        ) -> DispatchResult {
            ensure!(
                !EtwdReceipts::<T>::contains_key(receipt),
                Error::<T>::ReceiptAlreadyRecorded
            );
            EtwdReceipts::<T>::insert(receipt, amount);
            Self::deposit_event(Event::SettlementRecorded {
                payer: payer.clone(),
                receipt,
                amount,
            });
            Ok(())
        }
    }

    // ========================================================================
    // TreasurySettle — consumed by `pallet-tax` (SPEC §06/§08)
    // ========================================================================

    impl<T: Config> TreasurySettle<T::AccountId> for Pallet<T> {
        fn settle_fiat(payer: &T::AccountId, receipt: Hash32, amount: FiatAmount) -> DispatchResult {
            Pallet::<T>::do_record_settlement(payer, receipt, amount)
        }
    }
}
