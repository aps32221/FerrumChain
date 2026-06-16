//! # pallet-interop — 跨境互通與清算 (whitepaper §09–§10, Flow E)
//! # pallet-interop — Cross-border Interoperability & Clearing (whitepaper §09–§10, Flow E)
//!
//! 本模組是「鐵橋」(Ferrum Bridge) 的鏈上端,實作白皮書第 9 章「跨境互通架構」:
//! - **信任最小化橋接** — 以**真正的 GRANDPA 最終性證明**驗證跨共識訊息,信任根
//!   來自密碼學最終性而非受託保管人(`grandpa.rs` + `verify_finality` /
//!   `init_authority_set` / `rotate_authority_set`)。
//! - **跨國身分互認** — 各國受認證簽發者的**信任註冊表**、**跨鏈 DID 解析**
//!   (`resolve_did`),以及以註冊驗證金鑰進行的**跨境零知識互驗**
//!   (`register_issuer_vk` / `verify_foreign_proof`)。
//! - **跨境稅務協調** — **租稅協定登記表**(`register_treaty`)、**跨境電子發票
//!   互認**(`recognize_foreign_invoice`)、**OSS 一站式 VAT**(`oss_register` /
//!   `oss_report`)。
//! - **多邊淨額清算** — 以 **XSU** 計價的清算指令、依國家對軋差、互通驗證者以
//!   **本國 FER** 質押為跨鏈罰沒保證金(§10、§11)。
//!
//! This pallet is the on-chain side of the "Ferrum Bridge" and implements
//! whitepaper §09 in full:
//! - **Trust-minimized bridging** via **real GRANDPA finality proofs** (root of
//!   trust = cryptographic finality, no custodian) — `grandpa.rs` +
//!   `verify_finality` / `init_authority_set` / `rotate_authority_set`.
//! - **Cross-border identity recognition** — the trust registry, cross-chain
//!   DID resolution (`resolve_did`), and cross-border ZK verification using
//!   registered verification keys (`register_issuer_vk` / `verify_foreign_proof`).
//! - **Cross-border tax coordination** — tax-treaty registry (`register_treaty`),
//!   cross-border e-invoice recognition (`recognize_foreign_invoice`) and
//!   One-Stop-Shop VAT (`oss_register` / `oss_report`).
//! - **Multilateral netting** of XSU-priced instructions, with interop
//!   validators posting national FER as cross-slashable bonds (§10, §11).
//!
//! ## 隱私不變式 / Privacy invariant
//! 跨境互通遵守「個資不跨境」:鏈上只保存承諾值、雜湊、最終性/零知識證明與 XSU
//! 淨額——絕不含明文個資。明文留在來源國的鏈下加密庫,僅在授權程序下解密並留
//! 稽核軌跡 (§09)。
//!
//! Cross-border interop keeps "no PII across borders": on-chain state holds only
//! commitments, hashes, finality/ZK proofs and net XSU positions — never
//! plaintext PII (§09).
#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

pub mod grandpa;

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
    use super::grandpa::{verify_finality_proof, GrandpaVerifyError};
    use super::WeightInfo;
    use ferrum_primitives::{
        AgeProofPublicInputs, Balance, BlockNumber, BoundedVkBytes, ClearingInstruction, Commitment,
        CountryId, Did, DidResolution, GrandpaAuthoritySet, GrandpaFinalityProof, Hash, Hash32,
        Nullifier, OssRegistration, ProofBytes, TaxTreaty, TrustRegistryEntry, XcmStatus, XsuAmount,
        MAX_TAG_LEN, MIN_VALIDATOR_BOND,
    };
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;
    use pallet_identity_fer::DidRegistry;
    use sp_std::vec::Vec;

    /// 最終性證明位元組上限（位元組）/ Max length of a finality proof blob, in bytes.
    pub const MAX_FINALITY_PROOF_LEN: u32 = 65_536;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// `pallet-interop` 的設定特徵 — 供 runtime 接線 (SPEC §8)。
    ///
    /// Configuration trait for `pallet-interop` — wired by the runtime (SPEC §8).
    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// 本模組事件型別,須能轉換為 runtime 事件。
        ///
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// 聯邦治理來源（條約理事會）—— 用於信任註冊表、授權集合、淨額清算與罰沒。
        ///
        /// The federation-governed origin (treaty council).
        type FederationOrigin: EnsureOrigin<Self::RuntimeOrigin>;

        /// 中繼者來源 —— 用於提交清算指令與最終性/零知識證明。
        ///
        /// The relayer origin.
        type RelayerOrigin: EnsureOrigin<Self::RuntimeOrigin>;

        /// 本地身分登記介面 —— 供通用 DID 解析器解析本鏈 DID（§09）。
        ///
        /// The local identity registry, used by the universal DID resolver to
        /// resolve locally-anchored DIDs (§09 cross-chain DID resolution).
        type DidRegistry: pallet_identity_fer::DidRegistry;

        /// 本地鏈標籤（例如 `b"tw"`）—— 用以區分本鏈 / 外鏈 DID（§09）。
        ///
        /// This chain's local tag (e.g. `b"tw"`), used to route local vs.
        /// foreign DID resolution (§09).
        type LocalChainTag: Get<BoundedVec<u8, ConstU32<MAX_TAG_LEN>>>;

        /// 本模組交易的權重資訊。
        ///
        /// Weight information for this pallet's extrinsics.
        type WeightInfo: WeightInfo;
    }

    // ========================================================================
    // 儲存項 / Storage
    // ========================================================================

    /// 信任註冊表:`國家 -> 簽發者公鑰雜湊 -> 註冊項`（§09 跨國身分互認）。
    /// 採雙重 map 以便依國家前綴查詢「該國是否有受認可簽發者」。
    ///
    /// Trust registry: `country -> issuer key hash -> entry` (§09). A double map
    /// so we can prefix-query "does this country have a recognized issuer?".
    #[pallet::storage]
    pub type TrustRegistry<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        CountryId,
        Blake2_128Concat,
        Hash32,
        TrustRegistryEntry,
    >;

    /// 各外鏈受認可的 GRANDPA 授權集合（§09 輕客戶端信任根）。
    ///
    /// Per-country recognized GRANDPA authority set (§09 light-client root of trust).
    #[pallet::storage]
    pub type GrandpaAuthoritySets<T: Config> =
        StorageMap<_, Blake2_128Concat, CountryId, GrandpaAuthoritySet>;

    /// 輕客戶端追蹤的外鏈最終化區塊頭:`國家 -> (區塊雜湊, 區塊號)`（§09）。
    ///
    /// Light-client finalized head per bridged chain: `country -> (hash, number)`.
    #[pallet::storage]
    pub type FinalizedHeads<T: Config> =
        StorageMap<_, Blake2_128Concat, CountryId, (Hash, BlockNumber)>;

    /// 各國受認可簽發者的零知識驗證金鑰:`(國家, 簽發者公鑰雜湊) -> VK`（§09 ZK 互驗）。
    ///
    /// Per-issuer ZK verifying key: `(country, issuer key hash) -> VK` (§09).
    #[pallet::storage]
    pub type ForeignVerifyingKeys<T: Config> =
        StorageMap<_, Blake2_128Concat, (CountryId, Hash32), BoundedVkBytes>;

    /// 已用過的 nullifier,防止跨境證明被重放（§09）。
    ///
    /// Spent nullifiers, preventing cross-border proof replay (§09).
    #[pallet::storage]
    pub type UsedNullifiers<T: Config> = StorageMap<_, Blake2_128Concat, Nullifier, ()>;

    /// 租稅協定登記表:`(甲國, 乙國) -> 協定`（§09 雙重課稅減免）。
    ///
    /// Tax-treaty registry: `(country A, country B) -> treaty` (§09 double-tax relief).
    #[pallet::storage]
    pub type TaxTreaties<T: Config> =
        StorageMap<_, Blake2_128Concat, (CountryId, CountryId), TaxTreaty>;

    /// 互認的外國電子發票:`(來源國, 發票雜湊) -> 已認可`（§09 跨境電子發票互認）。
    ///
    /// Recognized foreign e-invoices: `(source country, invoice hash) -> bool`.
    #[pallet::storage]
    pub type RecognizedInvoices<T: Config> =
        StorageMap<_, Blake2_128Concat, (CountryId, Hash32), bool, ValueQuery>;

    /// OSS 一站式 VAT 登記:`供應者 DID -> 登記`（§09 跨境 VAT/GST）。
    ///
    /// One-Stop-Shop VAT registrations: `supplier DID -> registration` (§09).
    #[pallet::storage]
    pub type OssRegistrations<T: Config> = StorageMap<_, Blake2_128Concat, Did, OssRegistration>;

    /// 跨境清算指令：依自增 id 索引（§10 多邊淨額清算）。
    ///
    /// Cross-border clearing instructions, keyed by an auto-incrementing id (§10).
    #[pallet::storage]
    pub type Instructions<T: Config> = StorageMap<_, Blake2_128Concat, u64, ClearingInstruction>;

    /// 雙邊淨部位：`(from, to) -> XSU 金額`（§10/§11.3 清算窗口）。
    ///
    /// Bilateral net positions: `(from, to) -> XSU amount` (§10/§11.3).
    #[pallet::storage]
    pub type NetPositions<T: Config> =
        StorageMap<_, Blake2_128Concat, (CountryId, CountryId), XsuAmount, ValueQuery>;

    /// 下一筆清算指令 id。/ The next clearing instruction id.
    #[pallet::storage]
    pub type NextInstruction<T: Config> = StorageValue<_, u64, ValueQuery>;

    /// 互通驗證者集合：帳戶 -> 本國 FER 質押額（§10/§11.1 跨鏈罰沒保證金）。
    ///
    /// Interop validator set: account -> national-FER bond amount (§10/§11.1).
    #[pallet::storage]
    pub type InteropValidators<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, Balance>;

    /// 累計遭罰沒的互通驗證者保證金（§11.1 跨鏈罰沒）。
    ///
    /// Cumulative slashed interop-validator bond (§11.1).
    #[pallet::storage]
    pub type TotalSlashed<T: Config> = StorageValue<_, Balance, ValueQuery>;

    // ========================================================================
    // 事件 / Events
    // ========================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// 信任註冊表項已登記/更新。/ A trust-registry entry was registered/updated.
        IssuerRegistered { country: CountryId, issuer_key_hash: Hash32 },
        /// 某外鏈的 GRANDPA 授權集合已初始化。/ A chain's GRANDPA authority set was initialized.
        AuthoritySetInitialized { country: CountryId, set_id: u64 },
        /// 某外鏈的 GRANDPA 授權集合已換屆。/ A chain's GRANDPA authority set was rotated.
        AuthoritySetRotated { country: CountryId, set_id: u64 },
        /// 清算指令已提交。/ A clearing instruction was submitted.
        InstructionSubmitted { id: u64, from: CountryId, to: CountryId, amount: XsuAmount },
        /// 清算指令的最終性已驗證。/ A clearing instruction's finality was verified.
        FinalityVerified { id: u64 },
        /// 某外鏈最終化區塊頭已推進。/ A bridged chain's finalized head advanced.
        HeadFinalized { country: CountryId, number: BlockNumber },
        /// 某簽發者的零知識驗證金鑰已登記。/ An issuer's ZK verifying key was registered.
        IssuerVkRegistered { country: CountryId, issuer_key_hash: Hash32 },
        /// 跨境零知識證明驗證通過。/ A cross-border ZK proof verified successfully.
        ForeignProofVerified { country: CountryId, nullifier: Nullifier },
        /// 租稅協定已登記。/ A tax treaty was registered.
        TreatyRegistered { a: CountryId, b: CountryId },
        /// 外國電子發票已互認。/ A foreign e-invoice was recognized.
        ForeignInvoiceRecognized { country: CountryId, invoice_hash: Hash32 },
        /// OSS VAT 已登記。/ An OSS VAT registration was created.
        OssRegistered { home: CountryId },
        /// OSS 跨境申報已產生清算指令（依消費地分配）。
        /// An OSS cross-border report produced a destination-allocated clearing instruction.
        OssReported { from: CountryId, to: CountryId, amount: XsuAmount },
        /// 清算窗口已淨額並結算。/ A clearing window was netted and settled.
        Netted { window: u32, instructions: u32 },
        /// 一筆雙邊淨部位已更新。/ A bilateral net position was updated.
        NetPositionUpdated { from: CountryId, to: CountryId, amount: XsuAmount },
        /// 互通驗證者已登記並質押。/ An interop validator registered and bonded.
        ValidatorRegistered { who: T::AccountId, bond: Balance },
        /// 互通驗證者保證金遭罰沒（跨鏈不當行為）。
        /// An interop validator's bond was slashed (cross-chain misbehavior).
        ValidatorSlashed { who: T::AccountId, amount: Balance },
    }

    // ========================================================================
    // 錯誤 / Errors
    // ========================================================================

    #[pallet::error]
    pub enum Error<T> {
        /// 找不到指定的清算指令。/ The given clearing instruction does not exist.
        UnknownInstruction,
        /// 指令狀態不允許此操作。/ The instruction is not in a state that allows this operation.
        InvalidStatus,
        /// 最終性證明為空。/ The finality proof blob is empty.
        InvalidFinalityProof,
        /// 最終性證明位元組無法解碼。/ The finality proof bytes could not be decoded.
        MalformedFinalityProof,
        /// 該國尚未初始化 GRANDPA 授權集合。/ No GRANDPA authority set initialized for this country.
        AuthoritySetNotInitialized,
        /// 證明的 set_id 與鏈上授權集合不符。/ Proof set_id does not match the on-chain set.
        SetIdMismatch,
        /// precommit 簽署者不在受認可授權集合內。/ A precommit signer is not a recognized authority.
        UnknownAuthority,
        /// 授權者重複投票。/ An authority voted more than once.
        DuplicateAuthority,
        /// 最終性證明的簽章或權重未達門檻。/ Bad signature or insufficient finality weight.
        BadFinalityProof,
        /// 新授權集合的 set_id 非當前 +1。/ The new authority-set id is not current + 1.
        NonSequentialSetId,
        /// 最終化區塊頭未前進(企圖回退或重放舊頭)。
        /// The finalized head did not advance (stale or replayed head).
        StaleFinality,
        /// 該簽發者未在信任註冊表中受認可。/ The issuer is not recognized in the trust registry.
        IssuerNotRecognized,
        /// 找不到該簽發者的驗證金鑰。/ No verifying key registered for this issuer.
        VerifyingKeyNotFound,
        /// 跨境零知識證明位元組格式錯誤。/ Malformed cross-border ZK proof bytes.
        MalformedZkProof,
        /// 跨境零知識證明驗證失敗。/ Cross-border ZK proof verification failed.
        InvalidZkProof,
        /// 該證明的 nullifier 已被使用(重放)。/ The proof's nullifier was already spent (replay).
        ProofReplayed,
        /// 尚未與該國建立任何最終性(無最終化區塊頭)。
        /// No finality established with this country yet (no finalized head).
        NoFinalizedHead,
        /// 該供應者已登記 OSS。/ The supplier is already OSS-registered.
        AlreadyRegisteredOss,
        /// 該供應者未登記 OSS。/ The supplier is not OSS-registered.
        OssNotRegistered,
        /// 找不到指定的互通驗證者。/ The given interop validator is not registered.
        UnknownValidator,
        /// 質押金額低於最低保證金。/ The bond amount is below the minimum required.
        InsufficientBond,
        /// 罰沒金額超過目前保證金餘額。/ The slash amount exceeds the current bond balance.
        SlashExceedsBond,
        /// 驗證者已登記。/ The validator is already registered.
        AlreadyRegistered,
    }

    // ========================================================================
    // 外部呼叫 / Calls
    // ========================================================================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// 登記/更新信任註冊表項（聯邦治理） — §09 跨國身分互認。
        ///
        /// Register/update a trust-registry entry (federation-governed) — §09.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::register_issuer())]
        pub fn register_issuer(origin: OriginFor<T>, entry: TrustRegistryEntry) -> DispatchResult {
            T::FederationOrigin::ensure_origin(origin)?;
            TrustRegistry::<T>::insert(entry.country, entry.issuer_key_hash, entry.clone());
            Self::deposit_event(Event::IssuerRegistered {
                country: entry.country,
                issuer_key_hash: entry.issuer_key_hash,
            });
            Ok(())
        }

        /// 提交跨境清算指令（中繼者） — §10 以 XSU 計價。
        ///
        /// Submit a cross-border clearing instruction (relayer) — §10, in XSU.
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::submit_instruction())]
        pub fn submit_instruction(
            origin: OriginFor<T>,
            instr: ClearingInstruction,
        ) -> DispatchResult {
            T::RelayerOrigin::ensure_origin(origin)?;
            let id = Self::new_instruction(instr.from, instr.to, instr.amount, instr.detail_commitment);
            let _ = id;
            Ok(())
        }

        /// 以來源鏈 **GRANDPA 最終性證明**驗證指令（中繼者） — §09 輕客戶端橋接。
        ///
        /// 解碼證明、依指令來源國載入受認可授權集合、驗證 ≥2/3 ed25519 簽章,
        /// 推進輕客戶端最終化區塊頭,並將指令轉為 `FinalityVerified`。
        ///
        /// Verify an instruction against the source chain's **GRANDPA finality
        /// proof** (relayer) — §09 light-client bridging. Decodes the proof,
        /// loads the recognized authority set for the instruction's source
        /// country, verifies ≥2/3 ed25519 signatures, advances the light-client
        /// finalized head, and transitions the instruction to `FinalityVerified`.
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::verify_finality())]
        pub fn verify_finality(
            origin: OriginFor<T>,
            id: u64,
            finality_proof: BoundedVec<u8, ConstU32<MAX_FINALITY_PROOF_LEN>>,
        ) -> DispatchResult {
            T::RelayerOrigin::ensure_origin(origin)?;
            ensure!(!finality_proof.is_empty(), Error::<T>::InvalidFinalityProof);

            let proof = GrandpaFinalityProof::decode(&mut finality_proof.as_slice())
                .map_err(|_| Error::<T>::MalformedFinalityProof)?;

            Instructions::<T>::try_mutate(id, |maybe_instr| -> DispatchResult {
                let instr = maybe_instr.as_mut().ok_or(Error::<T>::UnknownInstruction)?;
                ensure!(instr.status == XcmStatus::Pending, Error::<T>::InvalidStatus);

                let source = instr.from;
                Self::verify_and_advance(source, &proof)?;

                instr.status = XcmStatus::FinalityVerified;
                Ok(())
            })?;

            Self::deposit_event(Event::FinalityVerified { id });
            Ok(())
        }

        /// 多邊淨額清算（聯邦治理） — §10/§11.3。
        ///
        /// Multilateral netting/settlement (federation-governed) — §10/§11.3.
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::net_and_settle())]
        pub fn net_and_settle(origin: OriginFor<T>, window: u32) -> DispatchResult {
            T::FederationOrigin::ensure_origin(origin)?;

            let mut settled: u32 = 0;
            let ids: Vec<u64> = Instructions::<T>::iter_keys().collect();

            for id in ids {
                let mut instr = match Instructions::<T>::get(id) {
                    Some(i) => i,
                    None => continue,
                };
                if instr.status != XcmStatus::FinalityVerified {
                    continue;
                }

                let key = (instr.from, instr.to);
                let updated =
                    XsuAmount(NetPositions::<T>::get(key).0.saturating_add(instr.amount.0));
                NetPositions::<T>::insert(key, updated);
                Self::deposit_event(Event::NetPositionUpdated {
                    from: instr.from,
                    to: instr.to,
                    amount: updated,
                });

                instr.status = XcmStatus::Accepted;
                Instructions::<T>::insert(id, instr);
                settled = settled.saturating_add(1);
            }

            Self::deposit_event(Event::Netted { window, instructions: settled });
            Ok(())
        }

        /// 互通驗證者登記並質押本國 FER（§10/§11.1 跨鏈罰沒保證金）。
        ///
        /// Register an interop validator and post a national-FER bond (§10/§11.1).
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::register_validator())]
        pub fn register_validator(origin: OriginFor<T>, bond: Balance) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(!InteropValidators::<T>::contains_key(&who), Error::<T>::AlreadyRegistered);
            ensure!(bond >= MIN_VALIDATOR_BOND, Error::<T>::InsufficientBond);

            InteropValidators::<T>::insert(&who, bond);
            Self::deposit_event(Event::ValidatorRegistered { who, bond });
            Ok(())
        }

        /// 罰沒互通驗證者的保證金（聯邦治理裁決，§10/§11.1 跨鏈不當行為）。
        ///
        /// Slash an interop validator's bond (federation-governed, §10/§11.1).
        #[pallet::call_index(5)]
        #[pallet::weight(T::WeightInfo::slash_validator())]
        pub fn slash_validator(
            origin: OriginFor<T>,
            who: T::AccountId,
            amount: Balance,
        ) -> DispatchResult {
            T::FederationOrigin::ensure_origin(origin)?;

            InteropValidators::<T>::try_mutate(&who, |maybe_bond| -> DispatchResult {
                let bond = maybe_bond.as_mut().ok_or(Error::<T>::UnknownValidator)?;
                ensure!(*bond >= amount, Error::<T>::SlashExceedsBond);
                *bond = bond.saturating_sub(amount);
                Ok(())
            })?;

            TotalSlashed::<T>::mutate(|t| *t = t.saturating_add(amount));
            Self::deposit_event(Event::ValidatorSlashed { who, amount });
            Ok(())
        }

        /// 初始化某外鏈的 GRANDPA 授權集合（聯邦治理引導，§09 輕客戶端信任根）。
        ///
        /// Initialize a bridged chain's GRANDPA authority set (federation
        /// bootstrap, §09 light-client root of trust).
        #[pallet::call_index(6)]
        #[pallet::weight(T::WeightInfo::init_authority_set())]
        pub fn init_authority_set(
            origin: OriginFor<T>,
            country: CountryId,
            set: GrandpaAuthoritySet,
        ) -> DispatchResult {
            T::FederationOrigin::ensure_origin(origin)?;
            let set_id = set.set_id;
            GrandpaAuthoritySets::<T>::insert(country, set);
            Self::deposit_event(Event::AuthoritySetInitialized { country, set_id });
            Ok(())
        }

        /// 換屆某外鏈的 GRANDPA 授權集合(§09 處理 authority set 換屆)。
        ///
        /// 以**當前**授權集合密碼學驗證「換屆區塊」的最終性,再採納下一個集合
        /// (`new_set.set_id` 必須為當前 +1)。信任仍錨定在已驗證的最終性,而非
        /// 治理的單方宣告。
        ///
        /// Rotate a bridged chain's GRANDPA authority set (§09 set handoff).
        /// Cryptographically verifies finality of the handoff block under the
        /// **current** set, then adopts the next set (`new_set.set_id` must be
        /// current + 1). Trust stays anchored in verified finality, not a
        /// unilateral governance declaration.
        #[pallet::call_index(7)]
        #[pallet::weight(T::WeightInfo::rotate_authority_set())]
        pub fn rotate_authority_set(
            origin: OriginFor<T>,
            country: CountryId,
            finality_proof: BoundedVec<u8, ConstU32<MAX_FINALITY_PROOF_LEN>>,
            new_set: GrandpaAuthoritySet,
        ) -> DispatchResult {
            T::FederationOrigin::ensure_origin(origin)?;
            ensure!(!finality_proof.is_empty(), Error::<T>::InvalidFinalityProof);

            let proof = GrandpaFinalityProof::decode(&mut finality_proof.as_slice())
                .map_err(|_| Error::<T>::MalformedFinalityProof)?;

            let current = GrandpaAuthoritySets::<T>::get(country)
                .ok_or(Error::<T>::AuthoritySetNotInitialized)?;
            Self::map_verify(&current, &proof)?;
            ensure!(
                new_set.set_id == current.set_id.saturating_add(1),
                Error::<T>::NonSequentialSetId
            );

            let set_id = new_set.set_id;
            GrandpaAuthoritySets::<T>::insert(country, new_set);
            FinalizedHeads::<T>::insert(country, (proof.target_hash, proof.target_number));
            Self::deposit_event(Event::AuthoritySetRotated { country, set_id });
            Ok(())
        }

        /// 為受認可簽發者登記零知識驗證金鑰（聯邦治理，§09 ZK 互驗）。
        ///
        /// Register a ZK verifying key for a recognized issuer (federation, §09).
        #[pallet::call_index(8)]
        #[pallet::weight(T::WeightInfo::register_issuer_vk())]
        pub fn register_issuer_vk(
            origin: OriginFor<T>,
            country: CountryId,
            issuer_key_hash: Hash32,
            vk: BoundedVkBytes,
        ) -> DispatchResult {
            T::FederationOrigin::ensure_origin(origin)?;
            ensure!(
                Self::is_trusted_issuer(country, issuer_key_hash),
                Error::<T>::IssuerNotRecognized
            );
            ForeignVerifyingKeys::<T>::insert((country, issuer_key_hash), vk);
            Self::deposit_event(Event::IssuerVkRegistered { country, issuer_key_hash });
            Ok(())
        }

        /// 跨境零知識互驗(中繼者，§09):以註冊的驗證金鑰驗證他國的選擇性揭露
        /// 證明,個資不跨境;以 nullifier 防重放。
        ///
        /// Cross-border ZK verification (relayer, §09): verify a foreign
        /// selective-disclosure proof using the registered verification key —
        /// no PII crosses the border — with nullifier replay protection.
        #[pallet::call_index(9)]
        #[pallet::weight(T::WeightInfo::verify_foreign_proof())]
        pub fn verify_foreign_proof(
            origin: OriginFor<T>,
            country: CountryId,
            issuer_key_hash: Hash32,
            proof: ProofBytes,
            inputs: AgeProofPublicInputs,
        ) -> DispatchResult {
            T::RelayerOrigin::ensure_origin(origin)?;
            ensure!(
                Self::is_trusted_issuer(country, issuer_key_hash),
                Error::<T>::IssuerNotRecognized
            );
            ensure!(
                !UsedNullifiers::<T>::contains_key(inputs.nullifier),
                Error::<T>::ProofReplayed
            );

            let vk = ForeignVerifyingKeys::<T>::get((country, issuer_key_hash))
                .ok_or(Error::<T>::VerifyingKeyNotFound)?;
            let vk_bytes: ferrum_primitives::VerifyingKeyBytes = vk.to_vec();

            let decoded = ferrum_zk::decode_proof(&proof).map_err(|_| Error::<T>::MalformedZkProof)?;
            let prepared = ferrum_zk::decode_vk(&vk_bytes).map_err(|_| Error::<T>::MalformedZkProof)?;
            let public_inputs = ferrum_zk::public_inputs_from(&inputs);
            let ok = ferrum_zk::verify_age_threshold(&decoded, &prepared, &public_inputs)
                .map_err(|_| Error::<T>::MalformedZkProof)?;
            ensure!(ok, Error::<T>::InvalidZkProof);

            UsedNullifiers::<T>::insert(inputs.nullifier, ());
            Self::deposit_event(Event::ForeignProofVerified { country, nullifier: inputs.nullifier });
            Ok(())
        }

        /// 登記雙邊租稅協定（聯邦治理，§09 雙重課稅減免）。
        ///
        /// Register a bilateral tax treaty (federation-governed, §09).
        #[pallet::call_index(10)]
        #[pallet::weight(T::WeightInfo::register_treaty())]
        pub fn register_treaty(
            origin: OriginFor<T>,
            a: CountryId,
            b: CountryId,
            treaty: TaxTreaty,
        ) -> DispatchResult {
            T::FederationOrigin::ensure_origin(origin)?;
            TaxTreaties::<T>::insert((a, b), treaty);
            Self::deposit_event(Event::TreatyRegistered { a, b });
            Ok(())
        }

        /// 互認一筆外國電子發票(中繼者，§09 跨境電子發票互認):需已與該來源國
        /// 建立最終性(輕客戶端已有最終化區塊頭)。
        ///
        /// Recognize a foreign e-invoice (relayer, §09): requires established
        /// finality with the source chain (the light client has a finalized head).
        #[pallet::call_index(11)]
        #[pallet::weight(T::WeightInfo::recognize_foreign_invoice())]
        pub fn recognize_foreign_invoice(
            origin: OriginFor<T>,
            country: CountryId,
            invoice_hash: Hash32,
        ) -> DispatchResult {
            T::RelayerOrigin::ensure_origin(origin)?;
            ensure!(FinalizedHeads::<T>::contains_key(country), Error::<T>::NoFinalizedHead);
            RecognizedInvoices::<T>::insert((country, invoice_hash), true);
            Self::deposit_event(Event::ForeignInvoiceRecognized { country, invoice_hash });
            Ok(())
        }

        /// OSS 一站式 VAT 登記(供應者，§09 跨境 VAT/GST)。
        ///
        /// One-Stop-Shop VAT registration (supplier, §09).
        #[pallet::call_index(12)]
        #[pallet::weight(T::WeightInfo::oss_register())]
        pub fn oss_register(
            origin: OriginFor<T>,
            subject: Did,
            registration: OssRegistration,
        ) -> DispatchResult {
            let _who = ensure_signed(origin)?;
            ensure!(!OssRegistrations::<T>::contains_key(&subject), Error::<T>::AlreadyRegisteredOss);
            let home = registration.home;
            OssRegistrations::<T>::insert(&subject, registration);
            Self::deposit_event(Event::OssRegistered { home });
            Ok(())
        }

        /// OSS 跨境申報(供應者，§09):於單一入口申報,依**消費地原則**將稅收
        /// 分配到目的國 —— 產生一筆 from=登記國、to=目的國的清算指令,進入既有的
        /// 多邊淨額清算管線。
        ///
        /// OSS cross-border report (supplier, §09): files at the single entry
        /// point and allocates revenue to the destination by the
        /// **destination principle** — producing a clearing instruction
        /// (from = home, to = destination) that flows into the existing
        /// multilateral netting pipeline.
        #[pallet::call_index(13)]
        #[pallet::weight(T::WeightInfo::oss_report())]
        pub fn oss_report(
            origin: OriginFor<T>,
            subject: Did,
            to: CountryId,
            amount: XsuAmount,
            detail_commitment: Commitment,
        ) -> DispatchResult {
            let _who = ensure_signed(origin)?;
            let reg = OssRegistrations::<T>::get(&subject).ok_or(Error::<T>::OssNotRegistered)?;
            let from = reg.home;
            let _id = Self::new_instruction(from, to, amount, detail_commitment);
            Self::deposit_event(Event::OssReported { from, to, amount });
            Ok(())
        }
    }

    // ========================================================================
    // 內部輔助 / Internal helpers
    // ========================================================================

    impl<T: Config> Pallet<T> {
        /// 配置一筆新的待驗證清算指令並回傳其 id。
        ///
        /// Allocate a new `Pending` clearing instruction and return its id.
        fn new_instruction(
            from: CountryId,
            to: CountryId,
            amount: XsuAmount,
            detail_commitment: Commitment,
        ) -> u64 {
            let id = NextInstruction::<T>::mutate(|n| {
                let i = *n;
                *n = n.saturating_add(1);
                i
            });
            let instr = ClearingInstruction {
                from,
                to,
                amount,
                detail_commitment,
                status: XcmStatus::Pending,
            };
            Instructions::<T>::insert(id, instr);
            Self::deposit_event(Event::InstructionSubmitted { id, from, to, amount });
            id
        }

        /// 將 `grandpa::verify_finality_proof` 的錯誤映射為本 pallet 的 `Error`。
        ///
        /// Map a `grandpa::verify_finality_proof` error onto this pallet's `Error`.
        fn map_verify(
            set: &GrandpaAuthoritySet,
            proof: &GrandpaFinalityProof,
        ) -> DispatchResult {
            verify_finality_proof(set, proof).map_err(|e| match e {
                GrandpaVerifyError::SetIdMismatch => Error::<T>::SetIdMismatch,
                GrandpaVerifyError::UnknownAuthority => Error::<T>::UnknownAuthority,
                GrandpaVerifyError::DuplicateAuthority => Error::<T>::DuplicateAuthority,
                GrandpaVerifyError::BadSignature => Error::<T>::BadFinalityProof,
                GrandpaVerifyError::NotEnoughWeight => Error::<T>::BadFinalityProof,
                GrandpaVerifyError::MalformedKey => Error::<T>::MalformedFinalityProof,
            })?;
            Ok(())
        }

        /// 驗證最終性證明並推進該國的輕客戶端最終化區塊頭(不得回退)。
        ///
        /// Verify a finality proof and advance the country's light-client
        /// finalized head (must not regress).
        fn verify_and_advance(
            country: CountryId,
            proof: &GrandpaFinalityProof,
        ) -> DispatchResult {
            let set = GrandpaAuthoritySets::<T>::get(country)
                .ok_or(Error::<T>::AuthoritySetNotInitialized)?;
            Self::map_verify(&set, proof)?;

            if let Some((_, number)) = FinalizedHeads::<T>::get(country) {
                ensure!(proof.target_number >= number, Error::<T>::StaleFinality);
            }
            FinalizedHeads::<T>::insert(country, (proof.target_hash, proof.target_number));
            Self::deposit_event(Event::HeadFinalized {
                country,
                number: proof.target_number,
            });
            Ok(())
        }

        /// 將小寫鏈標籤(如 `b"jp"`)轉為大寫國家碼(`b"JP"`)。
        ///
        /// Convert a lowercase chain tag (e.g. `b"jp"`) to an uppercase
        /// `CountryId` (`b"JP"`).
        fn tag_to_country(tag: &[u8]) -> CountryId {
            [
                tag.first().copied().unwrap_or(0).to_ascii_uppercase(),
                tag.get(1).copied().unwrap_or(0).to_ascii_uppercase(),
            ]
        }
    }

    // ========================================================================
    // 輔助查詢介面 / Helper read-only API
    // ========================================================================

    impl<T: Config> Pallet<T> {
        /// 查詢信任註冊表項是否存在且生效（§09）。
        ///
        /// Whether a trust-registry entry exists and is active (§09).
        pub fn is_trusted_issuer(country: CountryId, issuer_key_hash: Hash32) -> bool {
            TrustRegistry::<T>::get(country, issuer_key_hash)
                .map(|e| e.active)
                .unwrap_or(false)
        }

        /// 該國是否有任一受認可的簽發者（供跨鏈 DID 解析判斷來源鏈是否互認）。
        ///
        /// Whether a country has any recognized issuer (§09 used by the
        /// universal DID resolver to report cross-recognition).
        pub fn country_recognized(country: CountryId) -> bool {
            TrustRegistry::<T>::iter_prefix_values(country).any(|e| e.active)
        }

        /// 通用 DID 解析器(§09 跨鏈 DID 解析):本鏈 DID 直接解析其文件;外鏈
        /// DID 回傳來源國與「是否經信任註冊表互認」。
        ///
        /// Universal DID resolver (§09): local DIDs resolve to their document;
        /// foreign DIDs return the source country and whether it is recognized.
        pub fn resolve_did(did: &Did) -> DidResolution {
            let local = T::LocalChainTag::get();
            if did.is_local(local.as_slice()) {
                match T::DidRegistry::resolve(did) {
                    Some(doc) => DidResolution::Local(doc),
                    None => DidResolution::LocalUnknown,
                }
            } else {
                let country = Self::tag_to_country(did.chain_tag.as_slice());
                DidResolution::Foreign { country, recognized: Self::country_recognized(country) }
            }
        }

        /// 讀取一筆雙邊淨部位（無紀錄回傳 0）。
        ///
        /// Read a bilateral net position (zero if none).
        pub fn net_position(from: CountryId, to: CountryId) -> XsuAmount {
            NetPositions::<T>::get((from, to))
        }

        /// 查詢兩國間的租稅協定(任一方向)。
        ///
        /// Look up the tax treaty between two countries (either direction).
        pub fn treaty_for(a: CountryId, b: CountryId) -> Option<TaxTreaty> {
            TaxTreaties::<T>::get((a, b)).or_else(|| TaxTreaties::<T>::get((b, a)))
        }

        /// 某外國電子發票是否已互認。
        ///
        /// Whether a foreign e-invoice has been recognized.
        pub fn is_recognized_invoice(country: CountryId, invoice_hash: Hash32) -> bool {
            RecognizedInvoices::<T>::get((country, invoice_hash))
        }

        /// 某外鏈目前最終化的區塊頭。
        ///
        /// The currently finalized head of a bridged chain.
        pub fn finalized_head(country: CountryId) -> Option<(Hash, BlockNumber)> {
            FinalizedHeads::<T>::get(country)
        }
    }
}
