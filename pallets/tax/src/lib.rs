//! # pallet-tax — 稅務管理層 / Tax Administration Layer (whitepaper §06)
//!
//! 電子發票錨定、自動預扣、隱私保護的稅級證明與可授權稽核。稅務義務一律以
//! 法幣計價，實際繳納使用受監理的 CBDC（eTWD），絕不以波動性網路代幣計價。
//!
//! E-invoice anchoring, programmable withholding, privacy-preserving ZK
//! bracket proofs, and authorized audit. Tax obligations are ALWAYS
//! fiat-denominated; settlement happens in the regulated CBDC (eTWD) via
//! a `TreasurySettle`-style trait implemented by `pallet-treasury-fer`.
//! **No plaintext income or PII is ever stored
//! on-chain — only hashes/commitments** (whitepaper §03/§06 privacy
//! invariant).
//!
//! ## Cross-module wiring note
//! Per SPEC §06/§08, `Config::Treasury` is bound to a `TreasurySettle` trait.
//! Until the lead adds `ferrum_primitives::TreasurySettle`, this pallet
//! defines and exports [`pallet::TreasurySettle`] itself; `pallet-treasury-fer`
//! (or the runtime) implements it for the concrete treasury type and wires it
//! as `type Treasury = <TreasuryType>`.
//!
//! ## Core mechanisms (§06)
//! - **E-invoice anchoring** (`anchor_invoice`): invoice hash anchored
//!   on-chain in real time; line items stay encrypted off-chain.
//! - **Programmable withholding** (`withhold`): wages/interest withheld at
//!   payment time, recorded as a fiat-denominated obligation.
//! - **Filing** (`file_obligation`): citizens file obligations; basic filing
//!   is fee-free (subsidized by the treasury, §08).
//! - **ZK bracket proofs** (`prove_bracket`): prove "income is in bracket X"
//!   without revealing the amount, verified via `ferrum-zk`.
//! - **Settlement** (`settle`): pays the obligation in eTWD via
//!   `T::Treasury::settle_fiat`.
//! - **Authorized audit** (`authorize_audit`): records an access commitment
//!   for an invoice, forming an immutable audit trail.
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
    use ferrum_primitives::{
        Commitment, Did, FiatAmount, Hash32, InvoiceAnchor, TaxBracket, TaxKind, TaxObligation,
        AgeProofPublicInputs, ProofBytes,
    };
    use ferrum_zk::{decode_proof, decode_vk, public_inputs_from, verify_age_threshold};
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;
    use sp_std::vec::Vec;

    /// 國庫結算介面 — 由 `pallet-treasury-fer` 實作，本模組僅消費。
    ///
    /// Treasury settlement interface, implemented by `pallet-treasury-fer`;
    /// this pallet only consumes it (SPEC §06/§08 cross-module contract).
    pub trait TreasurySettle<AccountId> {
        /// 以法幣金額結算一筆稽核憑證所對應的義務（以 eTWD 支付）。
        ///
        /// Settle a fiat-denominated obligation referenced by `receipt`,
        /// paid in eTWD on behalf of `payer`.
        fn settle_fiat(payer: &AccountId, receipt: Hash32, amount: FiatAmount) -> DispatchResult;
    }

    /// 已結算稅收回呼 — 由消費端(如 `pallet-lottery`)實作,用於累計**經認證、
    /// 對應真實 eTWD 移轉**的稅收(僅 `settle` 路徑觸發,非無權限收據)。
    ///
    /// Authenticated-revenue hook — implemented by a consumer (e.g.
    /// `pallet-lottery`) to tally **settled, value-backed** tax revenue. Fired
    /// only from the `settle` path (never the permissionless receipt writer),
    /// so a permissionless caller cannot inflate the tally (SPEC §9 integration).
    pub trait RevenueSink {
        /// 通知一筆義務已以 eTWD 結算。/ Notify that an obligation settled in eTWD.
        fn note_settled(kind: TaxKind, amount: FiatAmount);
    }

    /// 無作用實作 — 供不接開獎系統的 runtime 與測試使用。
    /// No-op impl for runtimes/tests without a lottery consumer.
    impl RevenueSink for () {
        fn note_settled(_kind: TaxKind, _amount: FiatAmount) {}
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// Config trait — the public surface the runtime wires (SPEC §06).
    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// 事件類型 / The overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// 國庫結算介面（eTWD 繳納）。
        ///
        /// Treasury settlement trait implementation (eTWD payment, §08).
        type Treasury: TreasurySettle<Self::AccountId>;

        /// 已結算稅收回呼(預設 `()` 為無作用;開獎 runtime 接 `pallet-lottery`)。
        ///
        /// Authenticated-revenue hook (`()` for a no-op; lottery runtimes wire
        /// `pallet-lottery`). Fired on `settle` so the lottery's tax-proportional
        /// prize pool reflects only real, value-backed settlements (SPEC §9).
        type RevenueHook: RevenueSink;

        /// 稽核者來源 — 僅授權稽核員可呼叫 `authorize_audit`。
        ///
        /// Origin that may authorize an audit access (regulated auditors).
        type AuditorOrigin: EnsureOrigin<Self::RuntimeOrigin>;

        /// 治理來源 — 可更新稅級表（§06 ZK 稅級證明依據）。
        ///
        /// Governance origin allowed to update the tax bracket table.
        type GovernanceOrigin: EnsureOrigin<Self::RuntimeOrigin>;

        /// 權重資訊 / Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    // ========================================================================
    // Storage — 儲存項
    // ========================================================================

    /// 電子發票錨定表：發票雜湊 -> 錨定資料（明細留在鏈下加密儲存）。
    ///
    /// E-invoice anchors keyed by invoice hash. Line-item detail never
    /// touches the chain — only the hash and metadata are anchored (§06).
    #[pallet::storage]
    pub type Invoices<T: Config> = StorageMap<_, Blake2_128Concat, Hash32, InvoiceAnchor>;

    /// 發票錨定的**區塊高度**:發票雜湊 -> 區塊號(§06/§09 開獎期視窗化依據,
    /// 採區塊高度而非可被驗證者影響的時間戳)。
    ///
    /// Anchoring **block height** per invoice: hash -> block number. Used to
    /// window lottery draw periods by block height (not a validator-influenceable
    /// timestamp). SPEC §9 `InvoiceRegistry::anchored_block`.
    #[pallet::storage]
    pub type InvoiceAnchoredAt<T: Config> =
        StorageMap<_, Blake2_128Concat, Hash32, BlockNumberFor<T>>;

    /// 稅務義務表：(納稅人 DID, 申報期 slot) -> 義務記錄（以法幣計價）。
    ///
    /// Tax obligations keyed by `(subject Did, filing slot)`. Always
    /// fiat-denominated; detail is only a commitment (§06).
    #[pallet::storage]
    pub type Obligations<T: Config> =
        StorageMap<_, Blake2_128Concat, (Did, u64), TaxObligation>;

    /// 稅級表 — 供 ZK 稅級證明驗證時參照（§06）。
    ///
    /// The tax bracket table, referenced when verifying ZK bracket proofs.
    #[pallet::storage]
    pub type Brackets<T: Config> =
        StorageValue<_, BoundedVec<TaxBracket, ConstU32<32>>, ValueQuery>;

    /// 稽核日誌：發票雜湊 -> 存取承諾（檢視金鑰承諾），形成不可竄改稽核記錄。
    ///
    /// Audit log: invoice hash -> access commitment (viewing-key commitment),
    /// forming an immutable audit trail (§06 authorized audit).
    #[pallet::storage]
    pub type AuditLog<T: Config> = StorageMap<_, Blake2_128Concat, Hash32, Commitment>;

    // ========================================================================
    // Events — 事件
    // ========================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// 電子發票已錨定。Invoice anchored on-chain.
        ///
        /// `issuer` is the fixed `ferrum_primitives::AccountId` embedded in
        /// [`InvoiceAnchor`] (not `T::AccountId`) — `InvoiceAnchor` carries a
        /// concrete account id regardless of the runtime's account type.
        InvoiceAnchored { invoice_hash: Hash32, issuer: ferrum_primitives::AccountId, kind: TaxKind },
        /// 預扣稅額已記錄。Withholding recorded for a subject.
        Withheld { subject: Did, kind: TaxKind, amount: FiatAmount },
        /// 稅務義務已申報（基本申報免費，§08）。
        ObligationFiled { subject: Did, slot: u64, amount_due: FiatAmount },
        /// 稅級證明驗證通過。ZK bracket proof verified successfully.
        BracketProven { nullifier: ferrum_primitives::Nullifier },
        /// 稅務義務已以 eTWD 結算。Obligation settled in eTWD.
        Settled { subject: Did, slot: u64, amount: FiatAmount },
        /// 已授權對某發票的稽核存取。Audit access authorized for an invoice.
        AuditAuthorized { invoice: Hash32, viewing_key_commitment: Commitment },
    }

    // ========================================================================
    // Errors — 錯誤
    // ========================================================================

    #[pallet::error]
    pub enum Error<T> {
        /// 該發票雜湊已存在。Invoice hash already anchored.
        InvoiceAlreadyAnchored,
        /// 找不到該稅務義務。Obligation not found.
        ObligationNotFound,
        /// 該義務已結算。Obligation already settled.
        AlreadySettled,
        /// 稅級表已滿。Bracket table is full.
        TooManyBrackets,
        /// ZK 證明位元組格式錯誤。Malformed ZK proof bytes.
        MalformedProof,
        /// ZK 證明驗證失敗。ZK proof verification failed.
        InvalidProof,
        /// 找不到該發票，無法授權稽核。Invoice not found for audit authorization.
        InvoiceNotFound,
        /// 簽署者非發票所載的簽發商家。Signer is not the invoice's issuing merchant.
        NotIssuer,
    }

    // ========================================================================
    // Calls — 交易
    // ========================================================================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// 錨定電子發票雜湊（明細留在鏈下）。Anyone may anchor an invoice
        /// they issued; only the hash + metadata go on-chain (§06).
        ///
        /// 錨定電子發票（任何人皆可呼叫，發票明細不上鏈）。
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::anchor_invoice())]
        pub fn anchor_invoice(origin: OriginFor<T>, anchor: InvoiceAnchor) -> DispatchResult {
            let who = ensure_signed(origin)?;

            ensure!(
                !Invoices::<T>::contains_key(anchor.invoice_hash),
                Error::<T>::InvoiceAlreadyAnchored
            );
            // Only the issuing merchant may anchor its own invoice. This is
            // load-bearing for lottery-ticket integrity (§06/SPEC §9): a ticket
            // is eligible only if merchant-signed. `InvoiceAnchor.issuer` is a
            // fixed `ferrum_primitives::AccountId`; compare by SCALE encoding so
            // the check holds for any runtime `AccountId` that encodes alike.
            ensure!(who.encode() == anchor.issuer.encode(), Error::<T>::NotIssuer);

            let invoice_hash = anchor.invoice_hash;
            let issuer = anchor.issuer.clone();
            let kind = anchor.kind;

            Invoices::<T>::insert(invoice_hash, anchor);
            // Record the anchoring block height for lottery period windowing.
            InvoiceAnchoredAt::<T>::insert(invoice_hash, frame_system::Pallet::<T>::block_number());

            Self::deposit_event(Event::InvoiceAnchored { invoice_hash, issuer, kind });
            Ok(())
        }

        /// 對指定納稅人記錄一筆預扣稅額（§06 可程式化預扣）。
        ///
        /// Record a programmable withholding for `subject` (e.g. wage /
        /// interest payer withholds at source).
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::withhold())]
        pub fn withhold(
            origin: OriginFor<T>,
            subject: Did,
            kind: TaxKind,
            amount: FiatAmount,
        ) -> DispatchResult {
            let _who = ensure_signed(origin)?;

            Self::deposit_event(Event::Withheld { subject, kind, amount });
            Ok(())
        }

        /// 申報稅務義務（基本申報免費，由國庫補貼，§08）。
        ///
        /// File a tax obligation. Basic filing is fee-free — subsidized by
        /// the treasury subsidy fund (§08). The detail commitment is the
        /// only on-chain trace of the return contents.
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::file_obligation())]
        pub fn file_obligation(origin: OriginFor<T>, obligation: TaxObligation) -> DispatchResult {
            let _who = ensure_signed(origin)?;

            let subject = obligation.subject.clone();
            let amount_due = obligation.amount_due;
            // Filing slot is derived from the current block number so that
            // repeated filings within the same block do not silently
            // overwrite each other across different callers; the caller is
            // expected to pass a consistent slot via off-chain coordination.
            let slot = Self::current_slot();

            Obligations::<T>::insert((subject.clone(), slot), obligation);

            Self::deposit_event(Event::ObligationFiled { subject, slot, amount_due });
            Ok(())
        }

        /// 提交 ZK 稅級證明（重用 age-proof 的 Groth16 驗證形狀，§05/§06）。
        ///
        /// Submit a ZK bracket proof. Reuses the age-proof Groth16 shape:
        /// `[issuer_commitment, threshold, nullifier]` public inputs, verified
        /// via `ferrum-zk::verify_age_threshold`. No income amount is ever
        /// revealed — only that it falls within the proven bracket/threshold.
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::prove_bracket())]
        pub fn prove_bracket(
            origin: OriginFor<T>,
            proof: ProofBytes,
            inputs: AgeProofPublicInputs,
        ) -> DispatchResult {
            let _who = ensure_signed(origin)?;

            let decoded_proof = decode_proof(&proof).map_err(|_| Error::<T>::MalformedProof)?;

            // NOTE: the verifying key is provided by an off-chain/governed
            // source in a full deployment (e.g. a storage item populated by
            // governance). For this pallet's scope we accept an empty VK
            // bytes slice and rely on `ferrum-zk::decode_vk` to validate
            // shape; pallets/runtime wiring may extend this with a stored VK.
            let vk_bytes: ferrum_primitives::VerifyingKeyBytes = Vec::new();
            let vk = decode_vk(&vk_bytes).map_err(|_| Error::<T>::MalformedProof)?;

            let public_inputs = public_inputs_from(&inputs);

            let ok = verify_age_threshold(&decoded_proof, &vk, &public_inputs)
                .map_err(|_| Error::<T>::InvalidProof)?;
            ensure!(ok, Error::<T>::InvalidProof);

            Self::deposit_event(Event::BracketProven { nullifier: inputs.nullifier });
            Ok(())
        }

        /// 以 eTWD 結算指定納稅人於指定申報期的義務（§08）。
        ///
        /// Settle the obligation for `subject` at filing `slot`, paying in
        /// eTWD via `T::Treasury::settle_fiat`.
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::settle())]
        pub fn settle(origin: OriginFor<T>, subject: Did, slot: u64) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let mut obligation = Obligations::<T>::get((subject.clone(), slot))
                .ok_or(Error::<T>::ObligationNotFound)?;
            ensure!(!obligation.settled, Error::<T>::AlreadySettled);

            // Receipt commitment derived from the obligation's detail
            // commitment + slot, used as the treasury receipt key (Hash32).
            let receipt = obligation.detail_commitment;

            T::Treasury::settle_fiat(&who, receipt, obligation.amount_due)?;

            obligation.settled = true;
            let amount = obligation.amount_due;
            let kind = obligation.kind;
            Obligations::<T>::insert((subject.clone(), slot), obligation);

            // Notify the authenticated-revenue consumer (e.g. the lottery's
            // tax-proportional prize pool). Value-backed because we are on the
            // `settle` path that just moved eTWD (SPEC §9).
            T::RevenueHook::note_settled(kind, amount);

            Self::deposit_event(Event::Settled { subject, slot, amount });
            Ok(())
        }

        /// 授權對指定發票的稽核存取，記錄存取承諾（檢視金鑰承諾）。
        ///
        /// Authorize an audit access for `invoice`, recording the viewing-key
        /// commitment in [`AuditLog`] — an immutable audit trail (§06).
        /// Restricted to `T::AuditorOrigin`.
        #[pallet::call_index(5)]
        #[pallet::weight(T::WeightInfo::authorize_audit())]
        pub fn authorize_audit(
            origin: OriginFor<T>,
            invoice: Hash32,
            viewing_key_commitment: Commitment,
        ) -> DispatchResult {
            T::AuditorOrigin::ensure_origin(origin)?;

            ensure!(Invoices::<T>::contains_key(invoice), Error::<T>::InvoiceNotFound);

            AuditLog::<T>::insert(invoice, viewing_key_commitment);

            Self::deposit_event(Event::AuditAuthorized { invoice, viewing_key_commitment });
            Ok(())
        }

        /// 治理：設定稅級表（§06 ZK 稅級證明依據）。
        ///
        /// Governance-only: replace the tax bracket table.
        #[pallet::call_index(6)]
        #[pallet::weight(T::WeightInfo::set_brackets())]
        pub fn set_brackets(
            origin: OriginFor<T>,
            brackets: BoundedVec<TaxBracket, ConstU32<32>>,
        ) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;

            Brackets::<T>::put(brackets);
            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        /// 以目前區塊號作為申報期 slot 的衍生值。
        ///
        /// Derive a filing-period slot from the current block number.
        fn current_slot() -> u64 {
            use sp_runtime::SaturatedConversion;
            frame_system::Pallet::<T>::block_number().saturated_into::<u64>()
        }

        // ------- Public read API (consumed by the runtime's lottery adapter) -------

        /// 讀取一筆發票錨定。/ Read an invoice anchor.
        pub fn invoice(invoice_hash: &Hash32) -> Option<InvoiceAnchor> {
            Invoices::<T>::get(invoice_hash)
        }
        /// 該發票是否已錨定。/ Whether the invoice is anchored.
        pub fn is_anchored(invoice_hash: &Hash32) -> bool {
            Invoices::<T>::contains_key(invoice_hash)
        }
        /// 該發票的稅別。/ The invoice's tax category.
        pub fn invoice_kind(invoice_hash: &Hash32) -> Option<TaxKind> {
            Invoices::<T>::get(invoice_hash).map(|a| a.kind)
        }
        /// 該發票的錨定區塊高度。/ The invoice's anchoring block height.
        pub fn anchored_block(invoice_hash: &Hash32) -> Option<BlockNumberFor<T>> {
            InvoiceAnchoredAt::<T>::get(invoice_hash)
        }
    }
}
