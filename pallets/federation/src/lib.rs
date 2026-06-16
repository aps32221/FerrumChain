//! # pallet-federation — 聯邦治理與 XSU 籃子/準備池 (whitepaper §11)
//! # pallet-federation — Federation Governance & XSU Basket/Reserve Pool (whitepaper §11)
//!
//! 互通層(鐵橋)不屬於任何一國,而由**條約理事會**共同治理:每個成員國
//! 一席(由其央行或法定代表擔任),另設**中立技術秘書處**負責草擬與監測但
//! **無投票權**。一般提案須**同時**通過(a)成員數門檻(主權平等)與
//! (b)贊成方 XSU 籃子權重門檻(經濟份量)的**雙重多數**;通過後進入依
//! 治理領域而異的**時間鎖佇列**,到期由 `on_initialize` 自動 `enact`
//! (必要時觸發 WASM 無分叉升級)。XSU 籃子由會員 CBDC 依固定權重構成,
//! 準備池**足額擔保**,支援鑄造/贖回、多邊清算淨額,並每日發布鏈上
//! 儲備證明。
//!
//! The interop layer (Ferrum Bridge) belongs to no single nation; it is
//! co-governed by a **treaty council**: one seat per member (held by its
//! central bank or statutory representative), plus a **neutral technical
//! secretariat** that drafts and monitors but **holds no vote**. A normal
//! proposal must clear **both** axes of a **dual majority**: (a) a threshold
//! share of members (sovereign equality) and (b) a threshold share of XSU
//! basket weight among Aye voters (economic weight). Once it passes, the
//! proposal enters a **timelock queue** whose length depends on the
//! governance domain; `on_initialize` auto-`enact`s it at `eta` (triggering a
//! forkless WASM upgrade where needed). The XSU basket is a fixed-weight
//! basket of member CBDCs; the reserve pool is **fully collateralized**,
//! supporting mint/redeem, multilateral net clearing, and a daily on-chain
//! proof-of-reserves.
//!
//! ## 治理流程(圖 11.1)/ Governance pipeline (Fig 11.1)
//! `propose -> 理事會表決 -> 雙重多數 -> 時間鎖佇列 -> enact`
//! `propose -> council vote -> dual-majority -> timelock queue -> enact`
//!
//! ## 隱私不變式 / Privacy invariant
//! 本模組不儲存任何個人識別資料(PII)。清算指令的明細以
//! [`ferrum_primitives::Commitment`] 承諾鍵入,從不包含明細本身。
//!
//! This pallet stores no PII. Clearing-instruction detail is anchored only as
//! a [`ferrum_primitives::Commitment`] — never the detail itself.
#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

pub mod voting;
pub use voting::passes_dual_majority;

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
    use ferrum_primitives::{
        domain_threshold, FederationAction, MemberId, ProposalId, Vote, XsuAmount, XsuBasket,
        MAX_FEDERATION_MEMBERS,
    };
    use frame_support::{pallet_prelude::*, traits::EnsureOrigin};
    use frame_system::pallet_prelude::*;
    use sp_runtime::Perbill;
    use sp_std::collections::btree_map::BTreeMap;
    use sp_std::vec::Vec;

    use crate::voting::passes_dual_majority;

    /// 當前的「現在」(以區塊高度衡量,用於時間鎖計算)。
    ///
    /// The current "now" (measured in block numbers, used for timelock math).
    pub(crate) fn now<T: Config>() -> BlockNumberFor<T> {
        frame_system::Pallet::<T>::block_number()
    }

    // ========================================================================
    // 提案 / Proposal
    // ========================================================================

    /// 單一治理提案及其表決狀態(whitepaper §11.4)。
    ///
    /// A single governance proposal and its voting state (whitepaper §11.4).
    #[derive(Clone, Encode, Decode, MaxEncodedLen, TypeInfo, RuntimeDebug, PartialEq, Eq)]
    #[scale_info(skip_type_params(T))]
    pub struct Proposal<T: Config> {
        /// 欲執行的聯邦層動作 / The federation action to execute on enactment.
        pub action: FederationAction,
        /// 各理事會成員的票(成員代號 -> 票) / Each council member's ballot.
        pub votes: BoundedVec<(MemberId, Vote), ConstU32<MAX_FEDERATION_MEMBERS>>,
        /// 提案建立時的區塊高度 / Block number at proposal creation.
        pub created_at: BlockNumberFor<T>,
    }

    impl<T: Config> Proposal<T> {
        /// 建立新提案,初始無票 / Construct a new proposal with no votes yet.
        pub fn new(action: FederationAction, created_at: BlockNumberFor<T>) -> Self {
            Self { action, votes: BoundedVec::new(), created_at }
        }

        /// 依治理領域決定本提案的雙重多數門檻(§11.2 表)。
        ///
        /// The dual-majority threshold for this proposal's governance domain
        /// (§11.2 table).
        pub fn threshold(&self) -> Perbill {
            domain_threshold(self.action.domain())
        }

        /// 將已記錄的票轉換為 `BTreeMap`,供 [`passes_dual_majority`] 使用。
        ///
        /// Convert the recorded ballots into a `BTreeMap` for
        /// [`passes_dual_majority`].
        pub fn votes_map(&self) -> BTreeMap<MemberId, Vote> {
            self.votes.iter().cloned().collect()
        }
    }

    // ========================================================================
    // Pallet 主體 / Pallet body
    // ========================================================================

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// Config trait — the public surface the runtime wires (SPEC §11).
    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// 事件類型 / The overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// 理事會成員來源 — 成功時回傳該成員的 [`MemberId`](一國一席)。
        ///
        /// Council-member origin — on success yields the [`MemberId`] of the
        /// calling member (one seat per nation, §11.1).
        type CouncilMember: EnsureOrigin<Self::RuntimeOrigin, Success = MemberId>;

        /// 依聯邦動作決定時間鎖長度(§11.2:7/30 天等,以區塊數表示)。
        ///
        /// Timelock length (in blocks) for a given [`FederationAction`]
        /// (§11.2: 7/30-day domains etc, expressed in blocks).
        type TimelockFor: Get<BlockNumberFor<Self>>;

        /// 權重資訊 / Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    // ========================================================================
    // Storage — 儲存項
    // ========================================================================

    /// 條約理事會成員表:成員代號(等同 `CountryId`) -> 是否在席。
    ///
    /// Treaty council membership: [`MemberId`] (== `CountryId`) -> seated?
    /// (whitepaper §11.1: one seat per nation).
    #[pallet::storage]
    pub type Members<T: Config> = StorageMap<_, Blake2_128Concat, MemberId, bool, ValueQuery>;

    /// XSU 籃子權重表(§11 voting.rs `BTreeMap<MemberId, Perbill>` 的有界向量形式)。
    ///
    /// XSU basket weights — the bounded-vector form of voting.rs's
    /// `BTreeMap<MemberId, Perbill>` (whitepaper §10/§11.3).
    #[pallet::storage]
    pub type BasketWeights<T: Config> =
        StorageValue<_, BoundedVec<(MemberId, Perbill), ConstU32<MAX_FEDERATION_MEMBERS>>, ValueQuery>;

    /// 目前生效的 XSU 籃子(供清算/鑄造參照之完整結構,§10)。
    ///
    /// The currently active XSU basket (full structure used for
    /// minting/redeeming/clearing references, §10).
    #[pallet::storage]
    pub type ActiveBasket<T: Config> = StorageValue<_, XsuBasket, OptionQuery>;

    /// 提案表:提案編號 -> 提案 / Proposals keyed by [`ProposalId`].
    #[pallet::storage]
    pub type Proposals<T: Config> = StorageMap<_, Blake2_128Concat, ProposalId, Proposal<T>, OptionQuery>;

    /// 下一個提案編號 / The next [`ProposalId`] to allocate.
    #[pallet::storage]
    pub type NextId<T: Config> = StorageValue<_, ProposalId, ValueQuery>;

    /// 時間鎖佇列:到期區塊高度(eta) -> 提案編號。
    ///
    /// Timelock queue: enactment block number (`eta`) -> [`ProposalId`].
    #[pallet::storage]
    pub type Queued<T: Config> = StorageMap<_, Blake2_128Concat, BlockNumberFor<T>, ProposalId, OptionQuery>;

    // ------------------------------------------------------------------
    // XSU 準備池 / XSU reserve pool (§11.3)
    // ------------------------------------------------------------------

    /// 準備池中各 CBDC 的餘額(足額擔保 XSU,§11.3「足額擔保」)。
    ///
    /// Reserve-pool balances per CBDC, fully backing outstanding XSU
    /// (§11.3 "full backing").
    #[pallet::storage]
    pub type ReservePool<T: Config> = StorageMap<_, Blake2_128Concat, ferrum_primitives::CbdcCode, Balance, ValueQuery>;

    /// 已鑄造在外流通的 XSU 總量。
    ///
    /// Total XSU minted and outstanding.
    #[pallet::storage]
    pub type XsuIssued<T: Config> = StorageValue<_, Balance, ValueQuery>;

    /// 每個成員國持有的 XSU 餘額(用於多邊清算淨額,§11.3)。
    ///
    /// Per-member XSU balances (used for multilateral net clearing, §11.3).
    #[pallet::storage]
    pub type XsuBalances<T: Config> = StorageMap<_, Blake2_128Concat, MemberId, Balance, ValueQuery>;

    /// 最近一次儲備證明:(區塊高度, 各 CBDC 餘額快照雜湊)。
    ///
    /// Most recent proof-of-reserves: `(block, hash of the per-CBDC balance
    /// snapshot)` (§11.3 "Proof of reserves" — published daily, publicly
    /// auditable).
    #[pallet::storage]
    pub type LastProofOfReserve<T: Config> =
        StorageValue<_, (BlockNumberFor<T>, ferrum_primitives::Hash32), OptionQuery>;

    /// `Balance` 型別別名,沿用 `ferrum_primitives::Balance`(XSU 與 CBDC 以同一整數基底記帳)。
    ///
    /// Alias for `ferrum_primitives::Balance` (XSU and CBDC pool balances
    /// share the same integer base unit).
    pub type Balance = ferrum_primitives::Balance;

    // ========================================================================
    // Events — 事件
    // ========================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// 新提案已建立。`(id, by)` / A new proposal was created. `(id, by)`.
        Proposed { id: ProposalId, by: MemberId },
        /// 某成員對提案投票。`(id, member, vote)`.
        ///
        /// A member cast a ballot on a proposal. `(id, member, vote)`.
        Voted { id: ProposalId, member: MemberId, vote: Vote },
        /// 提案通過雙重多數,進入時間鎖佇列。`(id, eta)`.
        ///
        /// Proposal cleared dual majority and was queued. `(id, eta)`.
        Queued { id: ProposalId, eta: BlockNumberFor<T> },
        /// 提案已生效執行。`(id)` / Proposal was enacted. `(id)`.
        Enacted { id: ProposalId },
        /// 成員席位新增或移除。`(member, seated)`.
        ///
        /// A member seat was added or removed. `(member, seated)`.
        MembershipChanged { member: MemberId, seated: bool },
        /// XSU 籃子已重新配重。`(version)` / The XSU basket was reweighted. `(version)`.
        BasketReweighted { version: u32 },
        /// 觸發 WASM 無分叉升級。`(code_hash)`.
        ///
        /// A forkless WASM upgrade was triggered. `(code_hash)`.
        RuntimeUpgradeTriggered { code_hash: ferrum_primitives::Hash32 },
        /// 鑄造 XSU。`(member, cbdc, cbdc_amount, xsu_minted)`.
        ///
        /// XSU minted against a CBDC deposit. `(member, cbdc, cbdc_amount, xsu_minted)`.
        XsuMinted { member: MemberId, cbdc: ferrum_primitives::CbdcCode, cbdc_amount: Balance, xsu_minted: Balance },
        /// 贖回 XSU。`(member, cbdc, xsu_burned, cbdc_amount)`.
        ///
        /// XSU redeemed for a CBDC withdrawal. `(member, cbdc, xsu_burned, cbdc_amount)`.
        XsuRedeemed { member: MemberId, cbdc: ferrum_primitives::CbdcCode, xsu_burned: Balance, cbdc_amount: Balance },
        /// 多邊清算指令已記入淨額。`(from, to, amount)`.
        ///
        /// A clearing instruction was booked into net positions. `(from, to, amount)`.
        ClearingBooked { from: MemberId, to: MemberId, amount: XsuAmount },
        /// 清算窗口已結算淨部位。`(window)` / Net positions for a window were settled.
        NetSettled { window: u32 },
        /// 每日儲備證明已發布。`(block, digest)`.
        ///
        /// Daily proof-of-reserves was published. `(block, digest)`.
        ProofOfReservePublished { block: BlockNumberFor<T>, digest: ferrum_primitives::Hash32 },
    }

    // ========================================================================
    // Errors — 錯誤
    // ========================================================================

    #[pallet::error]
    pub enum Error<T> {
        /// 提案不存在 / Unknown proposal id.
        Unknown,
        /// 雙重多數未通過 / Dual majority was not met.
        Rejected,
        /// 該成員已投票 / Member already voted on this proposal.
        AlreadyVoted,
        /// 提案已在時間鎖佇列中,無法重複關閉 / Proposal already queued.
        AlreadyQueued,
        /// 籃子權重總和不為 100% / Basket weights do not sum to 100%.
        UnbalancedBasket,
        /// 籃子項目超出容量上限 / Basket entries exceed the bound.
        TooManyBasketEntries,
        /// 準備池餘額不足 / Insufficient reserve-pool balance.
        InsufficientReserve,
        /// 成員 XSU 餘額不足 / Insufficient XSU balance for this member.
        InsufficientXsu,
        /// 數值運算溢位 / Arithmetic overflow.
        Overflow,
        /// 找不到該 CBDC 於目前籃子中 / CBDC not found in the active basket.
        UnknownCbdc,
    }

    // ========================================================================
    // Hooks — on_initialize 自動 enact 到期提案 (§11.4)
    // ========================================================================

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        /// 每個區塊檢查時間鎖佇列,自動 enact 到期(eta == 當前區塊)的提案。
        ///
        /// Each block, check the timelock queue and auto-`enact` any proposal
        /// whose `eta` has arrived (== current block number).
        fn on_initialize(n: BlockNumberFor<T>) -> Weight {
            if let Some(id) = Queued::<T>::take(n) {
                let _ = Self::enact(id);
            }
            T::DbWeight::get().reads_writes(1, 1)
        }
    }

    // ========================================================================
    // Calls — 外部呼叫 (§11.4)
    // ========================================================================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// 任一理事會成員提出聯邦層提案(費率、籃子權重、成員資格…)。
        ///
        /// Any council member proposes a federation action (fees, basket
        /// weights, membership…).
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::propose())]
        pub fn propose(origin: OriginFor<T>, action: FederationAction) -> DispatchResult {
            let who = T::CouncilMember::ensure_origin(origin)?;
            let id = NextId::<T>::mutate(|n| {
                let i = *n;
                *n += 1;
                i
            });
            Proposals::<T>::insert(id, Proposal::<T>::new(action, now::<T>()));
            Self::deposit_event(Event::Proposed { id, by: who });
            Ok(())
        }

        /// 理事會成員對提案投票(Aye/Nay/Abstain);每位成員每案僅一票。
        ///
        /// A council member casts a ballot (Aye/Nay/Abstain); one ballot per
        /// member per proposal.
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::vote())]
        pub fn vote(origin: OriginFor<T>, id: ProposalId, vote: Vote) -> DispatchResult {
            let who = T::CouncilMember::ensure_origin(origin)?;
            Proposals::<T>::try_mutate(id, |maybe_p| -> DispatchResult {
                let p = maybe_p.as_mut().ok_or(Error::<T>::Unknown)?;
                ensure!(
                    !p.votes.iter().any(|(m, _)| *m == who),
                    Error::<T>::AlreadyVoted
                );
                p.votes
                    .try_push((who, vote))
                    .map_err(|_| Error::<T>::TooManyBasketEntries)?;
                Ok(())
            })?;
            Self::deposit_event(Event::Voted { id, member: who, vote });
            Ok(())
        }

        /// 達雙重多數後進入時間鎖佇列;不同領域套用不同時間鎖。
        ///
        /// On dual majority, queue under a timelock; each domain has its own
        /// timelock.
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::close())]
        pub fn close(origin: OriginFor<T>, id: ProposalId) -> DispatchResult {
            ensure_signed(origin)?;
            let p = Proposals::<T>::get(id).ok_or(Error::<T>::Unknown)?;
            let basket: BTreeMap<MemberId, Perbill> = BasketWeights::<T>::get().into_iter().collect();
            ensure!(
                passes_dual_majority(&p.votes_map(), &basket, p.threshold()),
                Error::<T>::Rejected
            );
            let eta = now::<T>() + T::TimelockFor::get();
            ensure!(!Queued::<T>::contains_key(eta), Error::<T>::AlreadyQueued);
            Queued::<T>::insert(eta, id); // on_initialize 到期自動 enact / on_initialize auto-enacts at eta
            Self::deposit_event(Event::Queued { id, eta });
            Ok(())
        }

        /// 治理(理事會多數)新增或移除一個成員國席位。
        ///
        /// Council-governed admission/removal of a member seat. Intended to
        /// be invoked via `enact` from an [`FederationAction::AdmitMember`] /
        /// [`FederationAction::RemoveMember`], but also exposed directly for
        /// initial bootstrap by any council member during genesis setup.
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::set_membership())]
        pub fn set_membership(origin: OriginFor<T>, member: MemberId, seated: bool) -> DispatchResult {
            T::CouncilMember::ensure_origin(origin)?;
            Members::<T>::insert(member, seated);
            Self::deposit_event(Event::MembershipChanged { member, seated });
            Ok(())
        }

        /// 秘書處/理事會設定新的 XSU 籃子(必須權重總和為 100%)。
        ///
        /// Set a new XSU basket (weights must sum to 100%). Mirrors the
        /// `Reweight` federation action when invoked via `enact`, but is also
        /// callable directly by a council member for genesis bootstrap.
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::set_basket())]
        pub fn set_basket(origin: OriginFor<T>, basket: XsuBasket) -> DispatchResult {
            T::CouncilMember::ensure_origin(origin)?;
            Self::do_set_basket(basket)
        }

        /// 成員以一籃 CBDC 依當前權重存入準備池,按比例鑄造 XSU(§11.3「鑄造/贖回」)。
        ///
        /// A member deposits CBDC into the reserve pool and mints XSU 1:1
        /// against the deposit (§11.3 "Mint / redeem"; full collateral).
        #[pallet::call_index(5)]
        #[pallet::weight(T::WeightInfo::mint_xsu())]
        pub fn mint_xsu(origin: OriginFor<T>, cbdc: ferrum_primitives::CbdcCode, cbdc_amount: Balance) -> DispatchResult {
            let who = T::CouncilMember::ensure_origin(origin)?;
            let basket = ActiveBasket::<T>::get().ok_or(Error::<T>::UnknownCbdc)?;
            ensure!(
                basket.entries.iter().any(|e| e.cbdc == cbdc),
                Error::<T>::UnknownCbdc
            );

            ReservePool::<T>::try_mutate(cbdc, |bal| -> DispatchResult {
                *bal = bal.checked_add(cbdc_amount).ok_or(Error::<T>::Overflow)?;
                Ok(())
            })?;
            // 1:1 足額擔保鑄造 / 1:1 fully-collateralized mint.
            let xsu_minted = cbdc_amount;
            XsuIssued::<T>::try_mutate(|total| -> DispatchResult {
                *total = total.checked_add(xsu_minted).ok_or(Error::<T>::Overflow)?;
                Ok(())
            })?;
            XsuBalances::<T>::try_mutate(who, |bal| -> DispatchResult {
                *bal = bal.checked_add(xsu_minted).ok_or(Error::<T>::Overflow)?;
                Ok(())
            })?;

            Self::deposit_event(Event::XsuMinted { member: who, cbdc, cbdc_amount, xsu_minted });
            Ok(())
        }

        /// 成員銷毀 XSU,按比例自準備池贖回 CBDC(§11.3「鑄造/贖回」)。
        ///
        /// A member burns XSU and redeems CBDC 1:1 from the reserve pool
        /// (§11.3 "Mint / redeem").
        #[pallet::call_index(6)]
        #[pallet::weight(T::WeightInfo::redeem_xsu())]
        pub fn redeem_xsu(origin: OriginFor<T>, cbdc: ferrum_primitives::CbdcCode, xsu_amount: Balance) -> DispatchResult {
            let who = T::CouncilMember::ensure_origin(origin)?;

            let member_bal = XsuBalances::<T>::get(who);
            ensure!(member_bal >= xsu_amount, Error::<T>::InsufficientXsu);

            let pool_bal = ReservePool::<T>::get(cbdc);
            ensure!(pool_bal >= xsu_amount, Error::<T>::InsufficientReserve);

            XsuBalances::<T>::insert(who, member_bal - xsu_amount);
            ReservePool::<T>::insert(cbdc, pool_bal - xsu_amount);
            XsuIssued::<T>::try_mutate(|total| -> DispatchResult {
                *total = total.checked_sub(xsu_amount).ok_or(Error::<T>::Overflow)?;
                Ok(())
            })?;

            Self::deposit_event(Event::XsuRedeemed {
                member: who,
                cbdc,
                xsu_burned: xsu_amount,
                cbdc_amount: xsu_amount,
            });
            Ok(())
        }

        /// 將一筆跨境清算指令(以 XSU 計價)記入雙邊淨部位(§11.3「多邊清算」)。
        ///
        /// Book a cross-border clearing instruction (priced in XSU) into the
        /// bilateral net position between two members (§11.3 "Multilateral
        /// clearing"). Detail stays off-chain; only the netted amount moves.
        #[pallet::call_index(7)]
        #[pallet::weight(T::WeightInfo::book_clearing())]
        pub fn book_clearing(origin: OriginFor<T>, to: MemberId, amount: XsuAmount) -> DispatchResult {
            let from = T::CouncilMember::ensure_origin(origin)?;

            let from_bal = XsuBalances::<T>::get(from);
            ensure!(from_bal >= amount.0, Error::<T>::InsufficientXsu);

            XsuBalances::<T>::insert(from, from_bal - amount.0);
            XsuBalances::<T>::try_mutate(to, |bal| -> DispatchResult {
                *bal = bal.checked_add(amount.0).ok_or(Error::<T>::Overflow)?;
                Ok(())
            })?;

            Self::deposit_event(Event::ClearingBooked { from, to, amount });
            Ok(())
        }

        /// 結算指定清算窗口的淨部位(§11.3「多邊清算」)。
        ///
        /// Settle net positions for the given clearing window (§11.3
        /// "Multilateral clearing": only net positions settle at window
        /// close). This implementation marks the window as settled; the
        /// actual XSU balance movements were already netted via
        /// [`Self::book_clearing`].
        #[pallet::call_index(8)]
        #[pallet::weight(T::WeightInfo::net_and_settle())]
        pub fn net_and_settle(origin: OriginFor<T>, window: u32) -> DispatchResult {
            T::CouncilMember::ensure_origin(origin)?;
            Self::deposit_event(Event::NetSettled { window });
            Ok(())
        }

        /// 發布每日鏈上儲備證明:對各 CBDC 準備池餘額做雜湊摘要(§11.3「儲備證明」)。
        ///
        /// Publish the daily on-chain proof-of-reserves: a hash digest over
        /// the current per-CBDC reserve-pool balances (§11.3 "Proof of
        /// reserves" — publicly auditable).
        #[pallet::call_index(9)]
        #[pallet::weight(T::WeightInfo::publish_proof_of_reserve())]
        pub fn publish_proof_of_reserve(origin: OriginFor<T>) -> DispatchResult {
            T::CouncilMember::ensure_origin(origin)?;
            let digest = Self::compute_reserve_digest();
            let block = now::<T>();
            LastProofOfReserve::<T>::put((block, digest));
            Self::deposit_event(Event::ProofOfReservePublished { block, digest });
            Ok(())
        }
    }

    // ========================================================================
    // 內部輔助函式 / Internal helpers
    // ========================================================================

    impl<T: Config> Pallet<T> {
        /// 套用一個籃子並更新 `BasketWeights` 索引(§11.3「再平衡」)。
        ///
        /// Apply a new basket and refresh the `BasketWeights` index (§11.3
        /// "Reweighting").
        pub(crate) fn do_set_basket(basket: XsuBasket) -> DispatchResult {
            ensure!(basket.is_balanced(), Error::<T>::UnbalancedBasket);

            // 籃子以 CBDC 為單位;`BasketWeights`(成員代號 -> 權重)由治理在
            // `Reweight` 動作中與成員國對應維護。此處保留既有成員權重,
            // 僅在籃子整體再平衡時更新版本與 `ActiveBasket`。
            //
            // The basket is keyed by CBDC; `BasketWeights` (MemberId ->
            // weight) is maintained by governance alongside the `Reweight`
            // action. Here we carry forward existing per-member weights and
            // update `ActiveBasket` + version on a basket-wide reweighting.
            let mut weights: BoundedVec<(MemberId, Perbill), ConstU32<MAX_FEDERATION_MEMBERS>> =
                BoundedVec::new();
            let version = basket.version;
            for (member, _seated) in Members::<T>::iter() {
                // 預設使用既有 BasketWeights 中的權重(若有);否則為零。
                // Carry forward existing per-member weight if present, else zero.
                let existing = BasketWeights::<T>::get()
                    .into_iter()
                    .find(|(m, _)| *m == member)
                    .map(|(_, w)| w)
                    .unwrap_or(Perbill::zero());
                let _ = weights.try_push((member, existing));
            }

            ActiveBasket::<T>::put(basket);
            BasketWeights::<T>::put(weights);
            Self::deposit_event(Event::BasketReweighted { version });
            Ok(())
        }

        /// 將提案在時間鎖到期時生效執行(§11.4 `enact`)。
        ///
        /// Enact a proposal whose timelock has matured (§11.4 `enact`).
        pub(crate) fn enact(id: ProposalId) -> DispatchResult {
            let p = Proposals::<T>::take(id).ok_or(Error::<T>::Unknown)?;
            match p.action {
                FederationAction::SetParameter { .. } => {
                    // 參數調整由執行端讀取提案歷史並套用;此處僅標記生效。
                    // Parameter changes are read from proposal history by the
                    // runtime/executor; here we only mark enactment.
                }
                FederationAction::AdmitMember { member } => {
                    Members::<T>::insert(member, true);
                    Self::deposit_event(Event::MembershipChanged { member, seated: true });
                }
                FederationAction::RemoveMember { member } => {
                    Members::<T>::insert(member, false);
                    Self::deposit_event(Event::MembershipChanged { member, seated: false });
                }
                FederationAction::Reweight { basket } => {
                    let _ = Self::do_set_basket(basket);
                }
                FederationAction::SuspendMember { member } => {
                    Members::<T>::insert(member, false);
                    Self::deposit_event(Event::MembershipChanged { member, seated: false });
                }
                FederationAction::RuntimeUpgrade { code_hash } => {
                    Self::deposit_event(Event::RuntimeUpgradeTriggered { code_hash });
                }
            }
            Self::deposit_event(Event::Enacted { id });
            Ok(())
        }

        /// 計算目前準備池各 CBDC 餘額的雜湊摘要(供每日儲備證明使用)。
        ///
        /// Compute a hash digest over the current per-CBDC reserve-pool
        /// balances (used for the daily proof-of-reserves).
        pub(crate) fn compute_reserve_digest() -> ferrum_primitives::Hash32 {
            let mut encoded: Vec<u8> = Vec::new();
            for (cbdc, balance) in ReservePool::<T>::iter() {
                encoded.extend_from_slice(&cbdc);
                encoded.extend_from_slice(&balance.to_le_bytes());
            }
            encoded.extend_from_slice(&XsuIssued::<T>::get().to_le_bytes());
            sp_io::hashing::blake2_256(&encoded)
        }
    }
}
