//! # pallet-lottery — 電子發票開獎 (whitepaper §06 subsection)
//! # pallet-lottery — E-invoice Lottery (whitepaper §06 subsection)
//!
//! 將已依 §06 錨定、**由商家簽署且對應真實已結算營業稅**的電子發票
//! (`invoice_hash`) 轉為**隱私彩券**,以**承諾—揭示並錨定於 GRANDPA 已最終化
//! 區塊雜湊**的方式抽獎,獎池規模為當期經認證稅收的治理比率(「稅務等比率」),
//! 並以央行鏈上認證之 eTWD 準備**原子化封頂扣減**,得獎以去識別化收據透過
//! `pallet-treasury-fer` 給付——真正價值移轉走 CBDC 軌道。
//!
//! Turns merchant-signed, payment-backed, already-anchored `pallet-tax`
//! e-invoices into **privacy tickets**, runs a **commit–reveal draw anchored to a
//! GRANDPA-finalized block hash**, sizes a fiat prize pool as a governed ratio of
//! authenticated period tax revenue ("tax-proportional"), clamps & atomically
//! debits it against the central bank's on-chain-attested eTWD reserve, and pays
//! winners with a PII-free receipt via `pallet-treasury-fer` — the value itself
//! moves on the CBDC rail.
//!
//! ## 隱私不變式 / Privacy invariant
//! 鏈上只保存承諾、雜湊、最終性錨定與 XSU/eTWD 帳務金額——絕不含明文個資。
//! On-chain state holds only commitments, hashes, finality anchors and eTWD
//! accounting amounts — never plaintext PII.
//!
//! ## Status
//! **DESIGN scaffold.** Bookkeeping and validation are implemented; the two ZK
//! circuits (eligibility + ownership) and the windowed entry-set / winner
//! selection are marked `TODO` and integrate via the [`traits`] below. See
//! `docs/einvoice-lottery-design.md` for the full design of record.
#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

pub mod traits;
pub mod types;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub use traits::{AttestedReserve, InvoiceRegistry, TreasuryPayout};
pub use types::*;

#[frame_support::pallet]
pub mod pallet {
    use super::traits::{AttestedReserve, InvoiceRegistry, TreasuryPayout};
    use super::types::*;
    use ferrum_primitives::{
        Balance, Commitment, FiatAmount, FiatCurrency, Hash32, Nullifier, ProofBytes, TaxKind,
    };
    use frame_support::pallet_prelude::*;
    use frame_support::traits::ReservableCurrency;
    use frame_system::pallet_prelude::*;
    use sp_runtime::traits::SaturatedConversion;
    use sp_std::vec::Vec;

    /// 萬分率分母 / parts-per-million denominator.
    pub const PPM: u32 = 1_000_000;

    /// 單期登記票數上限(BoundedVec 容量)。/ Max entries per draw.
    pub const MAX_ENTRIES: u32 = 4096;

    /// 每獎級每槽最大抽樣探測數(限制最終化權重)。
    /// Max rejection-sampling probes per tier slot (bounds finalize weight).
    pub const MAX_PROBES: u32 = 8;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// `pallet-lottery` 的設定特徵 / Configuration trait.
    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// 本模組事件型別。/ The overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// 唯讀存取 `pallet-tax` 的發票錨定(含錨定區塊高度)。
        /// Read-only access to `pallet-tax` invoice anchors (incl. anchoring block).
        type Tax: InvoiceRegistry;

        /// 年齡述詞門檻(資格電路公開輸入,如 18)。
        /// Age-predicate threshold for the eligibility circuit (e.g. 18).
        type AgeThreshold: Get<u32>;

        /// eTWD 得獎收據記錄器(`pallet-treasury-fer` 受限 origin 方法)。
        /// eTWD prize-receipt recorder (restricted-origin method on the treasury).
        type PrizeTreasury: TreasuryPayout<Self::AccountId>;

        /// 央行鏈上認證之 eTWD 準備餘額。/ The central-bank-attested eTWD reserve.
        type EtwdReserve: AttestedReserve;

        /// 治理來源(L4,§14)。/ The local L4 governance origin (§14).
        type GovernanceOrigin: EnsureOrigin<Self::RuntimeOrigin>;

        /// 緊急來源(央行 ⊕ 稽核者)。/ The break-glass emergency origin.
        type EmergencyOrigin: EnsureOrigin<Self::RuntimeOrigin>;

        /// 商家集合根管理者。/ The merchant-set-root registrar origin.
        type RegistrarOrigin: EnsureOrigin<Self::RuntimeOrigin>;

        /// 獎金計價幣別(例如 `b"TWD"`),每筆給付一律斷言相符。
        /// The prize currency (e.g. `b"TWD"`); every payout asserts equality.
        type PrizeCurrency: Get<FiatCurrency>;

        /// 憲制級資金比率上限(ppm)。/ Constitutional ceiling on the funding ratio (ppm).
        type MaxRatioPpm: Get<u32>;

        /// 有效抽獎所需之揭示法定數。/ Quorum of reveals required for a valid draw.
        type MinReveals: Get<u32>;

        /// 抽獎承諾保證金(僅驗證者)。/ The commit bond (validators only).
        type CommitDeposit: Get<Balance>;

        /// FER 貨幣 —— 用於保留/退還/罰沒抽獎承諾保證金(§11.1 跨鏈罰沒精神)。
        ///
        /// The FER currency used to reserve / return / slash the commit bond:
        /// committers reserve `CommitDeposit`, get it back on reveal, and forfeit
        /// it (with a fixed fallback seed folded into the entropy) if they fail to
        /// reveal — making commit-then-withhold strictly loss-making.
        type Currency: ReservableCurrency<Self::AccountId, Balance = Balance>;

        type WeightInfo: WeightInfo;
    }

    // ========================================================================
    // Storage
    // ========================================================================

    /// 下一期模板(治理設定)。/ The governed template for the next draw.
    #[pallet::storage]
    pub type ActiveConfig<T: Config> = StorageValue<_, DrawConfig, OptionQuery>;

    /// 各期不可變快照 + 狀態。/ Per-draw immutable snapshot + state.
    #[pallet::storage]
    pub type Draws<T: Config> = StorageMap<_, Blake2_128Concat, DrawId, DrawRecord>;

    /// 單調遞增的開獎 id。/ Monotonic draw id.
    #[pallet::storage]
    pub type NextDrawId<T: Config> = StorageValue<_, DrawId, ValueQuery>;

    /// 目前累計登記的開獎。/ The Open draw that entries accrue to.
    #[pallet::storage]
    pub type CurrentDraw<T: Config> = StorageValue<_, DrawId, ValueQuery>;

    /// 全域急停開關。/ Global kill-switch.
    #[pallet::storage]
    pub type Paused<T: Config> = StorageValue<_, bool, ValueQuery>;

    /// 已登記商家累加器根。/ Registered-merchant accumulator root.
    #[pallet::storage]
    pub type MerchantSetRoot<T: Config> = StorageValue<_, Hash32, OptionQuery>;

    /// 彩券:`invoice_hash -> 票`(主鍵去重 ⇒ 一發票一票)。
    /// Tickets: `invoice_hash -> ticket` (key reuse ⇒ one ticket per invoice).
    #[pallet::storage]
    pub type Tickets<T: Config> = StorageMap<_, Blake2_128Concat, Hash32, LotteryTicket>;

    /// 各期登記票數。/ Entries per draw.
    #[pallet::storage]
    pub type DrawTicketCount<T: Config> = StorageMap<_, Blake2_128Concat, DrawId, u32, ValueQuery>;

    /// 各期登記的 `invoice_hash` 有序集合(供封存與選號)。
    ///
    /// The per-draw entry set of registered `invoice_hash`es — the source of
    /// truth for sealing (Merkle root) and deterministic winner selection.
    #[pallet::storage]
    pub type DrawEntries<T: Config> =
        StorageMap<_, Blake2_128Concat, DrawId, BoundedVec<Hash32, ConstU32<MAX_ENTRIES>>, ValueQuery>;

    /// 登記防重放(canonical nullifier)。/ Registration anti-replay.
    #[pallet::storage]
    pub type EntryNullifiers<T: Config> = StorageMap<_, Blake2_128Concat, Nullifier, ()>;

    /// 領獎防重放(canonical nullifier)。/ Claim anti-replay.
    #[pallet::storage]
    pub type ClaimNullifiers<T: Config> = StorageMap<_, Blake2_128Concat, Nullifier, ()>;

    /// 各期合格 `invoice_hash` 排序集合之 Merkle 根。/ Merkle root of the sorted eligible set.
    #[pallet::storage]
    pub type EntryRoot<T: Config> = StorageMap<_, Blake2_128Concat, DrawId, Hash32>;

    /// 各期登記集大小。/ Size of the entry set.
    #[pallet::storage]
    pub type EntryCount<T: Config> = StorageMap<_, Blake2_128Concat, DrawId, u64, ValueQuery>;

    /// 每位參與者之承諾。/ Per-participant commit.
    #[pallet::storage]
    pub type Commits<T: Config> =
        StorageDoubleMap<_, Blake2_128Concat, DrawId, Blake2_128Concat, T::AccountId, Commit>;

    /// 已揭示 seed 之累進 XOR。/ Running XOR of revealed seeds.
    #[pallet::storage]
    pub type RevealedXor<T: Config> = StorageMap<_, Blake2_128Concat, DrawId, Hash32, ValueQuery>;

    /// 各期有效揭示數。/ Valid reveals per draw.
    #[pallet::storage]
    pub type RevealCount<T: Config> = StorageMap<_, Blake2_128Concat, DrawId, u32, ValueQuery>;

    /// 最終隨機數 `R`(公開)。/ Final randomness `R` (published).
    #[pallet::storage]
    pub type Entropy<T: Config> = StorageMap<_, Blake2_128Concat, DrawId, Hash32>;

    /// 各期、各獎級之中獎 `invoice_hash`。/ Winning hashes per draw/tier.
    #[pallet::storage]
    pub type Winners<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        DrawId,
        Blake2_128Concat,
        u8,
        BoundedVec<Hash32, ConstU32<1024>>,
        ValueQuery,
    >;

    /// 各期已認證稅收累計(期末凍結入 `DrawRecord`)。
    /// Running authenticated revenue per period (frozen into `DrawRecord` at period end).
    #[pallet::storage]
    pub type PeriodTaxRevenue<T: Config> =
        StorageMap<_, Blake2_128Concat, DrawId, FiatAmount, OptionQuery>;

    /// 各期已撥款獎池(eTWD 帳務計數器)。/ Funded eTWD accounting pool per draw.
    #[pallet::storage]
    pub type PrizePool<T: Config> = StorageMap<_, Blake2_128Concat, DrawId, FiatAmount, OptionQuery>;

    /// 領獎稽核:`receipt_key -> 已驗證檢視金鑰承諾`。/ Immutable claim audit.
    #[pallet::storage]
    pub type ClaimAudit<T: Config> = StorageMap<_, Blake2_128Concat, Hash32, Commitment>;

    /// 資格電路驗證金鑰(治理設定)。/ Governance-set eligibility-circuit VK.
    #[pallet::storage]
    pub type EligibilityVk<T: Config> = StorageValue<_, BoundedVec<u8, ConstU32<8192>>, OptionQuery>;

    /// 所有權電路驗證金鑰(治理設定)。/ Governance-set ownership-circuit VK.
    #[pallet::storage]
    pub type OwnershipVk<T: Config> = StorageValue<_, BoundedVec<u8, ConstU32<8192>>, OptionQuery>;

    /// 累計遭罰沒的承諾保證金(未揭示者)。/ Cumulative slashed commit bond (non-revealers).
    #[pallet::storage]
    pub type TotalSlashed<T: Config> = StorageValue<_, Balance, ValueQuery>;

    // ========================================================================
    // Genesis — 創世配置 (chain-spec 接線)
    // ========================================================================

    /// 創世配置:以 serde 友善的純量欄位描述開獎參數,於 `build` 組裝為鏈上
    /// `DrawConfig` 並選擇性開啟第一期。chain-spec 以 `"lottery": { … }` JSON 補丁設定。
    ///
    /// Genesis: serde-friendly scalar fields describing the lottery params,
    /// assembled into the on-chain `DrawConfig` in `build` (optionally opening the
    /// first draw). Set from the chain-spec via a `"lottery": { … }` JSON patch.
    #[pallet::genesis_config]
    #[derive(frame_support::DefaultNoBound)]
    pub struct GenesisConfig<T: Config> {
        pub period_blocks: u32,
        /// 合格發票種類(TaxKind 判別值;3 = ValueAdded)。/ Eligible kinds (TaxKind discriminants; 3 = ValueAdded).
        pub eligible_kinds: Vec<u8>,
        pub tax_ratio_ppm: u32,
        pub reserve_cap_ppm: u32,
        /// 各獎級 `(tier_id, share_ppm, winners, unit_cap_minor)`。/ Tiers.
        pub tiers: Vec<(u8, u32, u32, u128)>,
        pub allow_foreign: bool,
        pub commit_deadline: u32,
        pub reveal_deadline: u32,
        pub finalize_block: u32,
        pub claim_window: u32,
        pub merchant_set_root: Option<Hash32>,
        pub eligibility_vk: Vec<u8>,
        pub ownership_vk: Vec<u8>,
        /// 是否於創世立即開啟第一期。/ Open the first draw at genesis.
        pub open_first_draw: bool,
        #[serde(skip)]
        pub _phantom: core::marker::PhantomData<T>,
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            let kinds: Vec<TaxKind> =
                self.eligible_kinds.iter().filter_map(|k| tax_kind_from_u8(*k)).collect();
            let tiers: Vec<PrizeTier> = self
                .tiers
                .iter()
                .map(|(id, share, winners, cap)| PrizeTier {
                    tier_id: *id,
                    share_ppm: *share,
                    winners: *winners,
                    unit_cap: FiatAmount { currency: T::PrizeCurrency::get(), minor_units: *cap },
                })
                .collect();
            let cfg = DrawConfig {
                period_blocks: self.period_blocks,
                eligible_kinds: BoundedVec::try_from(kinds).unwrap_or_default(),
                tax_ratio_ppm: self.tax_ratio_ppm,
                reserve_cap_ppm: self.reserve_cap_ppm,
                tiers: BoundedVec::try_from(tiers).unwrap_or_default(),
                allow_foreign: self.allow_foreign,
                commit_deadline: self.commit_deadline,
                reveal_deadline: self.reveal_deadline,
                finalize_block: self.finalize_block,
                claim_window: self.claim_window,
            };
            ActiveConfig::<T>::put(cfg);
            if let Some(root) = self.merchant_set_root {
                MerchantSetRoot::<T>::put(root);
            }
            if let Ok(vk) = BoundedVec::try_from(self.eligibility_vk.clone()) {
                if !self.eligibility_vk.is_empty() {
                    EligibilityVk::<T>::put(vk);
                }
            }
            if let Ok(vk) = BoundedVec::try_from(self.ownership_vk.clone()) {
                if !self.ownership_vk.is_empty() {
                    OwnershipVk::<T>::put(vk);
                }
            }
            if self.open_first_draw {
                let _ = Pallet::<T>::do_open_draw();
            }
        }
    }

    /// 將 TaxKind 判別值映射為列舉(超出範圍回傳 None)。
    /// Map a TaxKind discriminant to the enum (out-of-range → None).
    fn tax_kind_from_u8(k: u8) -> Option<TaxKind> {
        Some(match k {
            0 => TaxKind::Income,
            1 => TaxKind::Wage,
            2 => TaxKind::Interest,
            3 => TaxKind::ValueAdded,
            4 => TaxKind::Withholding,
            5 => TaxKind::Other,
            _ => return None,
        })
    }

    // ========================================================================
    // Events
    // ========================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// 開獎已開啟。/ A draw opened.
        DrawOpened { draw: DrawId },
        /// 彩券已登記。/ A ticket was registered.
        TicketRegistered { invoice_hash: Hash32, draw: DrawId },
        /// 已提交承諾。/ A draw seed was committed.
        Committed { draw: DrawId, who: T::AccountId },
        /// 已揭示 seed。/ A draw seed was revealed.
        Revealed { draw: DrawId, who: T::AccountId },
        /// 期末已凍結稅收快照。/ The revenue snapshot was frozen at period end.
        RevenueSnapshotted { draw: DrawId, revenue: FiatAmount },
        /// 登記集已封存。/ The entry set was sealed.
        EntrySetSealed { draw: DrawId, count: u64 },
        /// 獎池已依稅務等比率撥款。/ The prize pool was funded (tax-proportional).
        PrizePoolFunded { draw: DrawId, revenue: FiatAmount, pool: FiatAmount },
        /// 獎池受準備上限封頂。/ The pool was clamped by the reserve cap.
        PrizePoolCapped { draw: DrawId, pool: FiatAmount },
        /// 抽獎已最終化。/ The draw was finalized.
        DrawCompleted { draw: DrawId, entropy: Hash32 },
        /// 未揭示者保證金遭罰沒,並折入固定後備種子。
        /// A non-revealer's bond was slashed and a fixed fallback seed folded in.
        SeedSlashed { draw: DrawId, who: T::AccountId, amount: Balance },
        /// 得獎已領取。/ A prize was claimed (keyed on receipt_key, never the winning hash).
        PrizeClaimed { receipt_key: Hash32, draw: DrawId, tier: u8, amount: FiatAmount },
        /// 不可分配餘額已回流準備。/ Unspendable residue recycled to reserve.
        PoolRecycled { draw: DrawId, amount: FiatAmount },
        /// 法定數未達,獎池已自動退回。/ Quorum missed; pool auto-refunded.
        AutoRefunded { draw: DrawId },
        /// 逾期未領已清掃回流。/ Expired prizes swept to reserve.
        ExpiredSwept { draw: DrawId, amount: FiatAmount },
        /// 開獎已暫停 / 恢復 / 取消。/ Lottery paused / resumed / cancelled.
        LotteryPaused,
        LotteryResumed,
        DrawCancelled { draw: DrawId },
        /// 驗證金鑰已設定。/ A circuit verifying key was set.
        EligibilityVkSet,
        OwnershipVkSet,
        /// 商家集合根已更新。/ The merchant-set root was updated.
        MerchantSetRootSet { root: Hash32 },
    }

    // ========================================================================
    // Errors
    // ========================================================================

    #[pallet::error]
    pub enum Error<T> {
        InvoiceNotAnchored,
        IneligibleInvoiceKind,
        DrawClosed,
        TicketAlreadyRegistered,
        NullifierUsed,
        NonCanonicalNullifier,
        InvalidProof,
        InvalidVk,
        ProofInputMismatch,
        MerchantRootUnset,
        NotAWinner,
        AlreadyClaimed,
        ClaimWindowClosed,
        InsufficientPool,
        CurrencyMismatch,
        RatioTooHigh,
        TiersNotExhaustive,
        UnknownDraw,
        WrongPhase,
        CommitClosed,
        AlreadyCommitted,
        BadReveal,
        RevealAfterFinalizeBlock,
        InsufficientReveals,
        EntropyExpired,
        RevenueNotSnapshotted,
        EntrySetNotSealed,
        TooEarlyToFinalize,
        NotYetExpired,
        InsufficientReserve,
        ConfigNotSet,
        LotteryIsPaused,
        /// 本期登記票數已達上限。/ The per-draw entry set is full.
        TooManyEntries,
        /// 餘額不足以保留承諾保證金。/ Insufficient balance to reserve the commit bond.
        InsufficientBalance,
    }

    // ========================================================================
    // Hooks — 週期驅動 / cadence driver
    // ========================================================================

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        /// 期末自動凍結稅收快照、將開期 `Open → Drawing`,並開啟下一期(O(1),
        /// 不在此處做封存/選號等重工)。
        ///
        /// At period end: freeze the revenue snapshot, transition the current
        /// draw `Open → Drawing`, and open the next period. Cheap and O(1) — the
        /// heavy sealing/selection happens in dedicated extrinsics.
        fn on_initialize(now: BlockNumberFor<T>) -> Weight {
            if Paused::<T>::get() {
                return Weight::from_parts(5_000_000, 0);
            }
            let now_u32: u32 = now.saturated_into();
            let draw = CurrentDraw::<T>::get();
            let Some(mut record) = Draws::<T>::get(draw) else {
                return Weight::from_parts(8_000_000, 0);
            };
            if record.state != DrawState::Open || now_u32 < record.period_end_block {
                return Weight::from_parts(8_000_000, 0);
            }
            let revenue = PeriodTaxRevenue::<T>::get(draw)
                .unwrap_or(FiatAmount { currency: T::PrizeCurrency::get(), minor_units: 0 });
            record.revenue_snapshot = Some(revenue);
            record.state = DrawState::Drawing;
            Draws::<T>::insert(draw, record);
            Self::deposit_event(Event::RevenueSnapshotted { draw, revenue });
            // Roll to the next period so new tickets accrue there.
            let _ = Self::do_open_draw();
            Weight::from_parts(40_000_000, 0)
        }
    }

    // ========================================================================
    // Calls
    // ========================================================================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// 登記彩券(免手續費,§06):需已錨定且為合格種類的發票,以 ZK 證明持有
        /// 該發票之購買秘密。/ Register a ticket (fee-free): anchored, eligible
        /// invoice + a ZK proof of possessing its purchase secret.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::register_ticket())]
        pub fn register_ticket(
            origin: OriginFor<T>,
            invoice_hash: Hash32,
            owner_commitment: Commitment,
            proof: ProofBytes,
            nullifier: Nullifier,
        ) -> DispatchResult {
            let _who = ensure_signed(origin)?;
            ensure!(!Paused::<T>::get(), Error::<T>::LotteryIsPaused);

            // (2) must already be anchored (post-hardening ⇒ merchant-signed).
            ensure!(T::Tax::is_anchored(&invoice_hash), Error::<T>::InvoiceNotAnchored);
            // (3) eligible kind.
            let kind = T::Tax::invoice_kind(&invoice_hash).ok_or(Error::<T>::InvoiceNotAnchored)?;
            let cfg = Self::current_config()?;
            ensure!(cfg.eligible_kinds.contains(&kind), Error::<T>::IneligibleInvoiceKind);
            // (4) window by anchoring BLOCK HEIGHT, never a Moment.
            let anchored = T::Tax::anchored_block(&invoice_hash).ok_or(Error::<T>::InvoiceNotAnchored)?;
            let draw = Self::derive_draw_by_block(anchored)?;
            ensure!(Self::phase(draw)? == Phase::Open, Error::<T>::DrawClosed);
            // (5) one ticket per invoice.
            ensure!(!Tickets::<T>::contains_key(invoice_hash), Error::<T>::TicketAlreadyRegistered);
            // (6) canonical, unused nullifier.
            ensure!(Self::is_canonical(&nullifier), Error::<T>::NonCanonicalNullifier);
            ensure!(!EntryNullifiers::<T>::contains_key(nullifier), Error::<T>::NullifierUsed);
            // (7) verify eligibility proof against the governance-set VK.
            let _root = MerchantSetRoot::<T>::get().ok_or(Error::<T>::MerchantRootUnset)?;
            Self::verify_eligibility(&invoice_hash, draw, &proof, &nullifier)?;

            Tickets::<T>::insert(
                invoice_hash,
                LotteryTicket { draw, owner_commitment, registered_at: Self::now() },
            );
            EntryNullifiers::<T>::insert(nullifier, ());
            DrawEntries::<T>::try_mutate(draw, |entries| {
                entries.try_push(invoice_hash).map_err(|_| Error::<T>::TooManyEntries)
            })?;
            DrawTicketCount::<T>::mutate(draw, |n| *n = n.saturating_add(1));
            Self::deposit_event(Event::TicketRegistered { invoice_hash, draw });
            Ok(())
        }

        /// 領獎(免手續費、可代送,§06):以 ZK 所有權電路證明持有中獎票並將受款帳戶
        /// 綁入公開輸入。/ Claim a prize (fee-free, relayable): a ZK ownership proof
        /// binding the payout beneficiary as a public input.
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::claim_prize())]
        pub fn claim_prize(
            origin: OriginFor<T>,
            draw_id: DrawId,
            tier: u8,
            invoice_hash: Hash32,
            beneficiary: T::AccountId,
            proof: ProofBytes,
            nullifier: Nullifier,
            viewing_key_commitment: Commitment,
        ) -> DispatchResult {
            let _who = ensure_signed(origin)?;
            ensure!(!Paused::<T>::get(), Error::<T>::LotteryIsPaused);

            let record = Draws::<T>::get(draw_id).ok_or(Error::<T>::UnknownDraw)?;
            ensure!(
                record.state == DrawState::Drawn || record.state == DrawState::Settled,
                Error::<T>::WrongPhase
            );
            // claim window (relative to the draw's finalize block).
            ensure!(
                Self::now() <= record.config.finalize_block.saturating_add(record.config.claim_window),
                Error::<T>::ClaimWindowClosed
            );
            ensure!(
                Winners::<T>::get(draw_id, tier).contains(&invoice_hash),
                Error::<T>::NotAWinner
            );
            ensure!(Self::is_canonical(&nullifier), Error::<T>::NonCanonicalNullifier);
            ensure!(!ClaimNullifiers::<T>::contains_key(nullifier), Error::<T>::AlreadyClaimed);

            let ticket = Tickets::<T>::get(invoice_hash).ok_or(Error::<T>::NotAWinner)?;
            // bind public inputs (invoice_hash, owner_commitment, draw_id, beneficiary).
            Self::verify_ownership(
                &invoice_hash,
                &ticket.owner_commitment,
                draw_id,
                &beneficiary,
                &proof,
                &nullifier,
                &viewing_key_commitment,
            )?;

            let amount = Self::per_winner(draw_id, &record, tier)?;
            ensure!(amount.currency == T::PrizeCurrency::get(), Error::<T>::CurrencyMismatch);
            PrizePool::<T>::try_mutate(draw_id, |maybe| -> DispatchResult {
                let pool = maybe.as_mut().ok_or(Error::<T>::InsufficientPool)?;
                ensure!(pool.currency == amount.currency, Error::<T>::CurrencyMismatch);
                pool.minor_units = pool
                    .minor_units
                    .checked_sub(amount.minor_units)
                    .ok_or(Error::<T>::InsufficientPool)?;
                Ok(())
            })?;

            // receipt key is fresh per-claim — never the raw winning hash.
            let receipt_key = Self::receipt_key(&invoice_hash, &nullifier);
            T::PrizeTreasury::credit_fiat(&beneficiary, receipt_key, amount)?;
            ClaimNullifiers::<T>::insert(nullifier, ());
            ClaimAudit::<T>::insert(receipt_key, viewing_key_commitment);
            Self::deposit_event(Event::PrizeClaimed { receipt_key, draw: draw_id, tier, amount });
            Ok(())
        }

        /// 設定/輪替資格電路 VK(治理)。/ Set/rotate the eligibility-circuit VK.
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::set_vk())]
        pub fn set_eligibility_vk(origin: OriginFor<T>, vk: BoundedVec<u8, ConstU32<8192>>) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;
            ensure!(!vk.is_empty(), Error::<T>::InvalidVk);
            EligibilityVk::<T>::put(vk);
            Self::deposit_event(Event::EligibilityVkSet);
            Ok(())
        }

        /// 設定/輪替所有權電路 VK(治理)。/ Set/rotate the ownership-circuit VK.
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::set_vk())]
        pub fn set_ownership_vk(origin: OriginFor<T>, vk: BoundedVec<u8, ConstU32<8192>>) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;
            ensure!(!vk.is_empty(), Error::<T>::InvalidVk);
            OwnershipVk::<T>::put(vk);
            Self::deposit_event(Event::OwnershipVkSet);
            Ok(())
        }

        /// 設定下一期模板(治理):驗證比率上限與分級和為 1。
        /// Set the next-draw template (governance): checks the ratio ceiling and Σtiers==1.
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::set_config())]
        pub fn set_config(origin: OriginFor<T>, config: DrawConfig) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;
            ensure!(config.tax_ratio_ppm <= T::MaxRatioPpm::get(), Error::<T>::RatioTooHigh);
            let sum: u32 = config.tiers.iter().map(|t| t.share_ppm).fold(0u32, |a, b| a.saturating_add(b));
            ensure!(sum == PPM, Error::<T>::TiersNotExhaustive);
            ActiveConfig::<T>::put(config);
            Ok(())
        }

        /// 更新商家集合根(註冊者)。/ Update the merchant-set root (registrar).
        #[pallet::call_index(5)]
        #[pallet::weight(T::WeightInfo::set_config())]
        pub fn set_merchant_set_root(origin: OriginFor<T>, root: Hash32) -> DispatchResult {
            T::RegistrarOrigin::ensure_origin(origin)?;
            MerchantSetRoot::<T>::put(root);
            Self::deposit_event(Event::MerchantSetRootSet { root });
            Ok(())
        }

        /// 開啟一期(治理):快照 `ActiveConfig` 為新的 `DrawRecord`。
        /// Open a draw (governance): snapshot `ActiveConfig` into a fresh record.
        #[pallet::call_index(6)]
        #[pallet::weight(T::WeightInfo::open_draw())]
        pub fn open_draw(origin: OriginFor<T>) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;
            Self::do_open_draw()
        }

        /// 提交開獎承諾(驗證者):保留 `CommitDeposit`。
        /// Commit a draw seed (validator): reserves `CommitDeposit`.
        #[pallet::call_index(7)]
        #[pallet::weight(T::WeightInfo::commit())]
        pub fn commit(origin: OriginFor<T>, draw_id: DrawId, commitment: Hash32) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let record = Draws::<T>::get(draw_id).ok_or(Error::<T>::UnknownDraw)?;
            ensure!(Self::now() <= record.config.commit_deadline, Error::<T>::CommitClosed);
            ensure!(!Commits::<T>::contains_key(draw_id, &who), Error::<T>::AlreadyCommitted);
            // Bond the commit deposit; forfeited if `who` never reveals.
            T::Currency::reserve(&who, T::CommitDeposit::get())
                .map_err(|_| Error::<T>::InsufficientBalance)?;
            Commits::<T>::insert(draw_id, &who, Commit { commitment, revealed: false });
            Self::deposit_event(Event::Committed { draw: draw_id, who });
            Ok(())
        }

        /// 揭示 seed(承諾者):須 `H(seed‖salt)==commitment` 且嚴格早於最終化區塊。
        /// Reveal a seed: requires `H(seed‖salt)==commitment` and strictly before `finalize_block`.
        #[pallet::call_index(8)]
        #[pallet::weight(T::WeightInfo::reveal())]
        pub fn reveal(
            origin: OriginFor<T>,
            draw_id: DrawId,
            seed: Hash32,
            salt: Hash32,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let record = Draws::<T>::get(draw_id).ok_or(Error::<T>::UnknownDraw)?;
            let now = Self::now();
            ensure!(now <= record.config.reveal_deadline, Error::<T>::BadReveal);
            // Anti last-revealer bias: reject at/after the entropy-anchor block.
            ensure!(now < record.config.finalize_block, Error::<T>::RevealAfterFinalizeBlock);
            Commits::<T>::try_mutate(draw_id, &who, |maybe| -> DispatchResult {
                let c = maybe.as_mut().ok_or(Error::<T>::BadReveal)?;
                ensure!(!c.revealed, Error::<T>::BadReveal);
                ensure!(Self::commit_hash(&seed, &salt) == c.commitment, Error::<T>::BadReveal);
                c.revealed = true;
                Ok(())
            })?;
            RevealedXor::<T>::mutate(draw_id, |acc| Self::xor_into(acc, &seed));
            RevealCount::<T>::mutate(draw_id, |n| *n = n.saturating_add(1));
            // Honest revealer reclaims the bond.
            T::Currency::unreserve(&who, T::CommitDeposit::get());
            Self::deposit_event(Event::Revealed { draw: draw_id, who });
            Ok(())
        }

        /// 封存登記集 Merkle 根(治理):須於期末且稅收快照存在後。
        /// Seal the entry-set Merkle root (governance): after period end and revenue snapshot.
        #[pallet::call_index(9)]
        #[pallet::weight(T::WeightInfo::seal_entry_set())]
        pub fn seal_entry_set(origin: OriginFor<T>, draw_id: DrawId) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;
            let record = Draws::<T>::get(draw_id).ok_or(Error::<T>::UnknownDraw)?;
            // Only after on_initialize has closed the period and frozen revenue.
            ensure!(record.state == DrawState::Drawing, Error::<T>::WrongPhase);
            ensure!(record.revenue_snapshot.is_some(), Error::<T>::RevenueNotSnapshotted);

            // Sort the entry set so anyone can recompute the same root and the
            // same winner indices from the public anchor set.
            let mut entries = DrawEntries::<T>::get(draw_id).into_inner();
            entries.sort_unstable();
            let count = entries.len() as u64;
            let mut buf = Vec::with_capacity(entries.len() * 32);
            for h in &entries {
                buf.extend_from_slice(h);
            }
            let root = sp_io::hashing::blake2_256(&buf);

            DrawEntries::<T>::insert(
                draw_id,
                BoundedVec::try_from(entries).map_err(|_| Error::<T>::TooManyEntries)?,
            );
            EntryRoot::<T>::insert(draw_id, root);
            EntryCount::<T>::insert(draw_id, count);
            Self::deposit_event(Event::EntrySetSealed { draw: draw_id, count });
            Ok(())
        }

        /// 依稅務等比率撥款獎池(治理):僅讀凍結快照,clamp+debit 原子化封頂。
        /// Fund the pool (governance): reads only the frozen snapshot; atomic clamp+debit.
        #[pallet::call_index(10)]
        #[pallet::weight(T::WeightInfo::fund_period())]
        pub fn fund_period(origin: OriginFor<T>, draw_id: DrawId) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;
            let record = Draws::<T>::get(draw_id).ok_or(Error::<T>::UnknownDraw)?;
            let revenue = record.revenue_snapshot.ok_or(Error::<T>::RevenueNotSnapshotted)?;
            ensure!(revenue.currency == T::PrizeCurrency::get(), Error::<T>::CurrencyMismatch);

            // Pool_raw = floor(revenue × r / PPM)
            let raw = mul_div_ppm(revenue.minor_units, record.config.tax_ratio_ppm);
            // ReserveCap = floor(attested × rc / PPM)
            let attested = T::EtwdReserve::attested_balance();
            ensure!(attested.currency == T::PrizeCurrency::get(), Error::<T>::CurrencyMismatch);
            let cap = mul_div_ppm(attested.minor_units, record.config.reserve_cap_ppm);
            let pool_minor = raw.min(cap);
            let pool = FiatAmount { currency: T::PrizeCurrency::get(), minor_units: pool_minor };

            // atomic with funding; fails closed if the attested reserve is insufficient.
            T::EtwdReserve::try_debit(pool).map_err(|_| Error::<T>::InsufficientReserve)?;
            PrizePool::<T>::insert(draw_id, pool);
            if raw > cap {
                Self::deposit_event(Event::PrizePoolCapped { draw: draw_id, pool });
            }
            Self::deposit_event(Event::PrizePoolFunded { draw: draw_id, revenue, pool });
            Ok(())
        }

        /// 最終化抽獎(無權限):須在最終化區塊之後且於 `BlockHashCount` 內,達法定揭示數。
        /// Finalize the draw (permissionless): after `finalize_block`, within the hash window, at quorum.
        #[pallet::call_index(11)]
        #[pallet::weight(T::WeightInfo::finalize_draw())]
        pub fn finalize_draw(origin: OriginFor<T>, draw_id: DrawId) -> DispatchResult {
            let _ = ensure_signed(origin)?;
            let config = Draws::<T>::try_mutate(draw_id, |maybe| -> Result<DrawConfig, DispatchError> {
                let record = maybe.as_mut().ok_or(Error::<T>::UnknownDraw)?;
                ensure!(record.state == DrawState::Drawing, Error::<T>::WrongPhase);
                let now = Self::now();
                ensure!(now > record.config.finalize_block, Error::<T>::TooEarlyToFinalize);
                ensure!(EntryRoot::<T>::contains_key(draw_id), Error::<T>::EntrySetNotSealed);
                ensure!(
                    RevealCount::<T>::get(draw_id) >= T::MinReveals::get(),
                    Error::<T>::InsufficientReveals
                );
                // The finalized block hash must still be in `frame_system`'s window,
                // else `block_hash` returns the default (zero) and the entropy is unsafe.
                let bhc: u32 = <T as frame_system::Config>::BlockHashCount::get().saturated_into();
                ensure!(
                    now <= record.config.finalize_block.saturating_add(bhc),
                    Error::<T>::EntropyExpired
                );
                record.state = DrawState::Drawn;
                Ok(record.config.clone())
            })?;

            // Non-revealers: fold a fixed published fallback seed and slash their
            // bond, so a withheld share cannot flip the result and is loss-making.
            let slashed = Self::fold_fallbacks_and_slash(draw_id);
            if slashed > 0 {
                TotalSlashed::<T>::mutate(|t| *t = t.saturating_add(slashed));
            }

            // R = blake2(⊕seedᵢ ‖ block_hash(finalize_block) ‖ draw_id).
            let r = Self::compute_entropy(draw_id, config.finalize_block);
            Entropy::<T>::insert(draw_id, r);
            // Deterministic winner selection over the sorted entry set.
            Self::select_winners(draw_id, &config, &r);
            Self::deposit_event(Event::DrawCompleted { draw: draw_id, entropy: r });
            Ok(())
        }

        /// 逾期清掃(無權限,期後):冪等回流殘餘獎池至準備。
        /// Sweep expired (permissionless, post-window): idempotently recycle residual pool.
        #[pallet::call_index(12)]
        #[pallet::weight(T::WeightInfo::sweep_expired())]
        pub fn sweep_expired(origin: OriginFor<T>, draw_id: DrawId) -> DispatchResult {
            let _ = ensure_signed(origin)?;
            Draws::<T>::try_mutate(draw_id, |maybe| -> DispatchResult {
                let record = maybe.as_mut().ok_or(Error::<T>::UnknownDraw)?;
                ensure!(
                    Self::now() > record.config.finalize_block.saturating_add(record.config.claim_window),
                    Error::<T>::NotYetExpired
                );
                if record.state != DrawState::Settled {
                    if let Some(residual) = PrizePool::<T>::take(draw_id) {
                        if residual.minor_units > 0 {
                            T::EtwdReserve::credit(residual);
                            Self::deposit_event(Event::ExpiredSwept { draw: draw_id, amount: residual });
                        }
                    }
                    record.state = DrawState::Settled;
                }
                Ok(())
            })
        }

        /// 急停(緊急來源)。/ Emergency pause.
        #[pallet::call_index(13)]
        #[pallet::weight(T::WeightInfo::pause())]
        pub fn emergency_pause(origin: OriginFor<T>) -> DispatchResult {
            T::EmergencyOrigin::ensure_origin(origin)?;
            Paused::<T>::put(true);
            Self::deposit_event(Event::LotteryPaused);
            Ok(())
        }

        /// 恢復(治理,刻意高於急停來源)。/ Resume (governance — deliberately higher than pause).
        #[pallet::call_index(14)]
        #[pallet::weight(T::WeightInfo::pause())]
        pub fn resume(origin: OriginFor<T>) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;
            Paused::<T>::put(false);
            Self::deposit_event(Event::LotteryResumed);
            Ok(())
        }
    }

    // ========================================================================
    // Internal helpers
    // ========================================================================

    /// 票券生命週期的衍生視圖。/ Derived lifecycle view of a draw.
    #[derive(PartialEq, Eq)]
    pub enum Phase {
        Open,
        Drawing,
        Settled,
    }

    impl<T: Config> Pallet<T> {
        fn now() -> ferrum_primitives::BlockNumber {
            frame_system::Pallet::<T>::block_number().saturated_into()
        }

        /// 由稅務結算回呼累計當期已認證稅收(僅 eTWD,同幣別)。供 runtime 轉接器
        /// 在 `pallet-tax::settle` 觸發 `ValueAdded` 時呼叫(§9 認證營收)。
        ///
        /// Tally authenticated period revenue from the tax-settlement hook.
        /// Called by the runtime adapter on a `ValueAdded` `pallet-tax::settle`.
        pub fn note_settled_revenue(amount: FiatAmount) {
            let draw = CurrentDraw::<T>::get();
            PeriodTaxRevenue::<T>::mutate(draw, |maybe| match maybe {
                Some(acc) if acc.currency == amount.currency => {
                    acc.minor_units = acc.minor_units.saturating_add(amount.minor_units);
                }
                Some(_) => {}
                None => *maybe = Some(amount),
            });
        }

        fn current_config() -> Result<DrawConfig, Error<T>> {
            ActiveConfig::<T>::get().ok_or(Error::<T>::ConfigNotSet)
        }

        /// 將 `DrawState` 投影為票券視圖階段。/ Project `DrawState` to a ticketing phase.
        fn phase(draw: DrawId) -> Result<Phase, Error<T>> {
            let record = Draws::<T>::get(draw).ok_or(Error::<T>::UnknownDraw)?;
            Ok(match record.state {
                DrawState::Pending | DrawState::Open => Phase::Open,
                DrawState::Drawing | DrawState::Drawn => Phase::Drawing,
                DrawState::Settled | DrawState::Cancelled => Phase::Settled,
            })
        }

        /// 由錨定區塊高度推導其所屬開獎期。/ Derive the draw period from an anchoring block.
        fn derive_draw_by_block(_anchored_block: ferrum_primitives::BlockNumber) -> Result<DrawId, Error<T>> {
            // The currently-open draw collects entries; the on_initialize driver
            // rolls to the next period. A full implementation maps the block into
            // [period_start, period_end) windows.
            Ok(CurrentDraw::<T>::get())
        }

        fn do_open_draw() -> DispatchResult {
            let config = Self::current_config()?;
            let id = NextDrawId::<T>::mutate(|n| {
                let i = *n;
                *n = n.saturating_add(1);
                i
            });
            let start = Self::now();
            let record = DrawRecord {
                config: config.clone(),
                period_start_block: start,
                period_end_block: start.saturating_add(config.period_blocks),
                revenue_snapshot: None,
                state: DrawState::Open,
            };
            Draws::<T>::insert(id, record);
            CurrentDraw::<T>::put(id);
            Self::deposit_event(Event::DrawOpened { draw: id });
            Ok(())
        }

        fn per_winner(draw_id: DrawId, record: &DrawRecord, tier: u8) -> Result<FiatAmount, Error<T>> {
            let t = record
                .config
                .tiers
                .iter()
                .find(|t| t.tier_id == tier)
                .ok_or(Error::<T>::WrongPhase)?;
            let pool = PrizePool::<T>::get(draw_id).map(|p| p.minor_units).unwrap_or(0);
            // Tier_k = floor(pool × w_k / PPM); payout = min(floor(Tier/n), cap).
            let tier_amount = mul_div_ppm(pool, t.share_ppm);
            let n = core::cmp::max(t.winners as u128, 1);
            let per = (tier_amount / n).min(t.unit_cap.minor_units);
            Ok(FiatAmount { currency: T::PrizeCurrency::get(), minor_units: per })
        }

        fn receipt_key(invoice_hash: &Hash32, nullifier: &Nullifier) -> Hash32 {
            let mut buf = Vec::with_capacity(64);
            buf.extend_from_slice(invoice_hash);
            buf.extend_from_slice(nullifier);
            sp_io::hashing::blake2_256(&buf)
        }

        fn commit_hash(seed: &Hash32, salt: &Hash32) -> Hash32 {
            let mut buf = Vec::with_capacity(64);
            buf.extend_from_slice(seed);
            buf.extend_from_slice(salt);
            sp_io::hashing::blake2_256(&buf)
        }

        fn xor_into(acc: &mut Hash32, seed: &Hash32) {
            for i in 0..32 {
                acc[i] ^= seed[i];
            }
        }

        /// 對未揭示的承諾者:折入固定後備種子並罰沒其保證金,回傳罰沒總額。
        ///
        /// For committers who never revealed: fold a fixed published fallback seed
        /// into `RevealedXor` and slash the reserved bond. Returns the total slashed.
        fn fold_fallbacks_and_slash(draw_id: DrawId) -> Balance {
            let deposit = T::CommitDeposit::get();
            let non_revealers: Vec<T::AccountId> = Commits::<T>::iter_prefix(draw_id)
                .filter(|(_, c)| !c.revealed)
                .map(|(who, _)| who)
                .collect();
            let mut total: Balance = 0;
            for who in non_revealers {
                // fixed, published fallback — deterministic, not chooseable by `who`
                let fb = Self::fallback_seed(draw_id, &who);
                RevealedXor::<T>::mutate(draw_id, |acc| Self::xor_into(acc, &fb));
                let _ = T::Currency::slash_reserved(&who, deposit);
                total = total.saturating_add(deposit);
                Commits::<T>::mutate(draw_id, &who, |maybe| {
                    if let Some(c) = maybe {
                        c.revealed = true; // mark processed (idempotent on re-finalize attempts)
                    }
                });
                Self::deposit_event(Event::SeedSlashed { draw: draw_id, who, amount: deposit });
            }
            total
        }

        /// 為某承諾者推導固定後備種子。/ Derive a fixed fallback seed for a committer.
        fn fallback_seed(draw_id: DrawId, who: &T::AccountId) -> Hash32 {
            let mut buf = Vec::new();
            buf.extend_from_slice(b"ferrum-fallback");
            buf.extend_from_slice(&draw_id.to_le_bytes());
            buf.extend_from_slice(&who.encode());
            sp_io::hashing::blake2_256(&buf)
        }

        /// `R = blake2_256(⊕seedᵢ ‖ block_hash(finalize_block) ‖ draw_id)`.
        fn compute_entropy(draw_id: DrawId, finalize_block: ferrum_primitives::BlockNumber) -> Hash32 {
            let xored = RevealedXor::<T>::get(draw_id);
            let fb: BlockNumberFor<T> = (finalize_block as u64).saturated_into();
            let block_hash = frame_system::Pallet::<T>::block_hash(fb);
            let mut buf = Vec::with_capacity(96);
            buf.extend_from_slice(&xored);
            buf.extend_from_slice(block_hash.as_ref());
            buf.extend_from_slice(&draw_id.to_le_bytes());
            sp_io::hashing::blake2_256(&buf)
        }

        /// 以最終隨機數 `R` 對排序後的登記集做確定性、不放回(有界探測)選號,
        /// 將各獎級中獎 `invoice_hash` 寫入 `Winners`。任何人皆可獨立重算。
        ///
        /// Deterministic, sampling-without-replacement (bounded-probe) winner
        /// selection over the sorted entry set, keyed by the final randomness `R`.
        /// Writes per-tier winning `invoice_hash`es to `Winners`; recomputable by anyone.
        fn select_winners(draw_id: DrawId, config: &DrawConfig, r: &Hash32) {
            let entries = DrawEntries::<T>::get(draw_id);
            let count = entries.len() as u64;
            if count == 0 {
                return;
            }
            for tier in config.tiers.iter() {
                let n = core::cmp::min(tier.winners as u64, count);
                let mut chosen: BoundedVec<Hash32, ConstU32<1024>> = BoundedVec::new();
                let mut used = sp_std::collections::btree_set::BTreeSet::<u64>::new();
                let max_probes = n.saturating_mul(MAX_PROBES as u64).saturating_add(MAX_PROBES as u64);
                let mut slot: u64 = 0;
                let mut probes: u64 = 0;
                while (chosen.len() as u64) < n && probes < max_probes {
                    let idx = Self::draw_index(r, draw_id, tier.tier_id, slot, count);
                    slot = slot.saturating_add(1);
                    probes = probes.saturating_add(1);
                    if used.insert(idx) {
                        let _ = chosen.try_push(entries[idx as usize]);
                    }
                }
                if !chosen.is_empty() {
                    Winners::<T>::insert(draw_id, tier.tier_id, chosen);
                }
            }
        }

        /// `idx = blake2_256(R ‖ "ferrum-draw" ‖ draw_id ‖ tier ‖ slot)[..16] mod count`。
        fn draw_index(r: &Hash32, draw_id: DrawId, tier_id: u8, slot: u64, count: u64) -> u64 {
            let mut buf = Vec::with_capacity(64);
            buf.extend_from_slice(r);
            buf.extend_from_slice(b"ferrum-draw");
            buf.extend_from_slice(&draw_id.to_le_bytes());
            buf.push(tier_id);
            buf.extend_from_slice(&slot.to_le_bytes());
            let h = sp_io::hashing::blake2_256(&buf);
            let mut x = [0u8; 16];
            x.copy_from_slice(&h[..16]);
            let v = u128::from_le_bytes(x);
            (v % (count as u128)) as u64
        }

        /// nullifier 必須為 canonical 的 BLS12-381 純量表示(值 < 場模數),確保鏈上
        /// 位元組與電路綁定的場元素一一對應,杜絕非正規重編碼重放。
        ///
        /// The nullifier must be a canonical BLS12-381 scalar (value < field
        /// modulus), so its on-chain bytes map bijectively to the field element the
        /// circuit binds — blocking replay via a non-canonical re-encoding.
        fn is_canonical(nullifier: &Nullifier) -> bool {
            ferrum_zk::is_canonical_scalar(nullifier)
        }

        /// 驗證資格電路證明:以治理設定的 VK,綁定 invoice_hash / 商家集合根 /
        /// draw_id / 門檻 / nullifier 為公開輸入(`ferrum-zk` 資格電路)。
        ///
        /// Verify the eligibility proof against the governance-set VK, binding
        /// `invoice_hash` / merchant_set_root / `draw_id` / threshold / nullifier
        /// as public inputs (the `ferrum-zk` eligibility circuit).
        fn verify_eligibility(
            invoice_hash: &Hash32,
            draw: DrawId,
            proof: &ProofBytes,
            nullifier: &Nullifier,
        ) -> DispatchResult {
            let vk = EligibilityVk::<T>::get().ok_or(Error::<T>::InvalidVk)?;
            let root = MerchantSetRoot::<T>::get().ok_or(Error::<T>::MerchantRootUnset)?;
            let ok = ferrum_zk::lottery_eligibility::verify(
                proof,
                &vk.to_vec(),
                invoice_hash,
                &root,
                draw,
                T::AgeThreshold::get(),
                nullifier,
            )
            .map_err(|_| Error::<T>::InvalidProof)?;
            ensure!(ok, Error::<T>::InvalidProof);
            Ok(())
        }

        /// 驗證所有權/領獎電路證明:綁定 invoice_hash / owner_commitment / draw_id /
        /// **受款帳戶** / nullifier / 檢視金鑰承諾為公開輸入(`ferrum-zk` 所有權電路)。
        ///
        /// Verify the ownership-claim proof against the governance-set VK, binding
        /// invoice_hash / owner_commitment / draw_id / **beneficiary** / nullifier /
        /// viewing-key commitment as public inputs (the `ferrum-zk` ownership circuit).
        #[allow(clippy::too_many_arguments)]
        fn verify_ownership(
            invoice_hash: &Hash32,
            owner_commitment: &Commitment,
            draw_id: DrawId,
            beneficiary: &T::AccountId,
            proof: &ProofBytes,
            nullifier: &Nullifier,
            viewing_key_commitment: &Commitment,
        ) -> DispatchResult {
            let vk = OwnershipVk::<T>::get().ok_or(Error::<T>::InvalidVk)?;
            let beneficiary_bytes = beneficiary.encode();
            let ok = ferrum_zk::lottery_ownership::verify(
                proof,
                &vk.to_vec(),
                invoice_hash,
                owner_commitment,
                draw_id,
                &beneficiary_bytes,
                nullifier,
                viewing_key_commitment,
            )
            .map_err(|_| Error::<T>::InvalidProof)?;
            ensure!(ok, Error::<T>::InvalidProof);
            Ok(())
        }
    }

    /// `floor(value × ppm / PPM)` in u128.
    fn mul_div_ppm(value: u128, ppm: u32) -> u128 {
        value.saturating_mul(ppm as u128) / (PPM as u128)
    }

    /// 本模組交易的權重資訊。/ Weight information for this pallet's extrinsics.
    pub trait WeightInfo {
        fn register_ticket() -> Weight;
        fn claim_prize() -> Weight;
        fn set_vk() -> Weight;
        fn set_config() -> Weight;
        fn open_draw() -> Weight;
        fn commit() -> Weight;
        fn reveal() -> Weight;
        fn seal_entry_set() -> Weight;
        fn fund_period() -> Weight;
        fn finalize_draw() -> Weight;
        fn sweep_expired() -> Weight;
        fn pause() -> Weight;
    }

    /// 預設權重(原型;待 `frame-benchmarking` 取代)。
    /// Default weights (prototype; replace with `frame-benchmarking`).
    impl WeightInfo for () {
        fn register_ticket() -> Weight { Weight::from_parts(40_000_000, 0) }
        fn claim_prize() -> Weight { Weight::from_parts(45_000_000, 0) }
        fn set_vk() -> Weight { Weight::from_parts(15_000_000, 0) }
        fn set_config() -> Weight { Weight::from_parts(15_000_000, 0) }
        fn open_draw() -> Weight { Weight::from_parts(20_000_000, 0) }
        fn commit() -> Weight { Weight::from_parts(20_000_000, 0) }
        fn reveal() -> Weight { Weight::from_parts(20_000_000, 0) }
        fn seal_entry_set() -> Weight { Weight::from_parts(20_000_000, 0) }
        fn fund_period() -> Weight { Weight::from_parts(25_000_000, 0) }
        fn finalize_draw() -> Weight { Weight::from_parts(60_000_000, 0) }
        fn sweep_expired() -> Weight { Weight::from_parts(20_000_000, 0) }
        fn pause() -> Weight { Weight::from_parts(10_000_000, 0) }
    }
}
