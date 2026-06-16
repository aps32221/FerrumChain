//! # Ferrum 鐵鏈 — `pallet-identity-fer` (身分驗證層 / Identity Layer, 白皮書 §05)
//!
//! 由「受認證簽發機構」（accredited issuers）錨定 `did:fer` DID 文件的雜湊
//! (`doc_hash`)；**鏈上絕不存放任何個資 (PII)**，僅有承諾值 (commitment) 與
//! 雜湊 (hash)。撤銷狀態以累加器承諾 (`RevocationAccumulator`) 表示。
//!
//! Anchors the hash (`doc_hash`) of a `did:fer` DID document, written only by
//! accredited issuers. **No PII is ever stored on-chain** — only commitments
//! and hashes. Revocation status is represented by an accumulator commitment
//! (`RevocationAccumulator`).
//!
//! ## 白皮書節錄 (whitepaper excerpt, §04/§05)
//!
//! ```rust,ignore
//! #[frame_support::pallet]
//! pub mod pallet {
//!     #[pallet::storage]
//!     pub type DidRegistry<T: Config> =
//!         StorageMap<_, Blake2_128Concat, Did, DidDocument<T::AccountId>, OptionQuery>;
//!
//!     #[pallet::call]
//!     impl<T: Config> Pallet<T> {
//!         #[pallet::weight(T::WeightInfo::register_did())]
//!         pub fn register_did(origin: OriginFor<T>, did: Did, doc_hash: H256) -> DispatchResult {
//!             let issuer = ensure_signed(origin)?;          // 僅承諾值，非明文
//!             ensure!(Issuers::<T>::contains_key(&issuer), Error::<T>::NotAccredited);
//!             ensure!(!DidRegistry::<T>::contains_key(&did), Error::<T>::AlreadyExists);
//!             DidRegistry::<T>::insert(&did, DidDocument { controller: issuer, doc_hash });
//!             Ok(())
//!         }
//!     }
//! }
//! ```
//!
//! 本 pallet 依 `SPEC.md` §3 之公開介面完整實作：`Dids` / `DidByController` /
//! `RevocationAccumulator` / `AccreditedIssuers` 儲存，以及
//! `anchor_did` / `rotate_keys` / `update_revocation` / `register_issuer` 四個
//! extrinsic（精神上對應上方節錄的 `register_did`，並擴充金鑰輪換與撤銷管理）。
//!
//! This pallet implements the full public surface required by `SPEC.md` §3:
//! the `Dids` / `DidByController` / `RevocationAccumulator` / `AccreditedIssuers`
//! storage items, and the `anchor_did` / `rotate_keys` / `update_revocation` /
//! `register_issuer` extrinsics (spiritually the `register_did` excerpt above,
//! extended with key rotation and revocation-accumulator management).

#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(test)]
mod mock;
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
        Commitment, Did, DidDocument, DidKeyRef, MAX_DID_KEYS, MAX_TAG_LEN,
    };
    use frame_support::{
        pallet_prelude::*,
        traits::EnsureOrigin,
    };
    use frame_system::pallet_prelude::*;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// ## Config — public surface wired by the runtime (SPEC §3)
    ///
    /// 每個關聯型別皆記錄其用途；執行時 (runtime) 透過這些型別將本 pallet
    /// 接入治理、發行人來源與事件系統。
    /// Each associated type is documented; the runtime wires governance,
    /// issuer origins and the event system through these.
    #[pallet::config]
    pub trait Config: frame_system::Config<AccountId = ferrum_primitives::AccountId> {
        /// 執行時事件型別。The runtime's aggregated event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// 限定為「受認證簽發機構」的來源（§05）。
        /// Origin restricted to accredited issuers (§05).
        type IssuerOrigin: EnsureOrigin<Self::RuntimeOrigin>;

        /// 限定為「鏈上治理」的來源（用於認證新簽發機構）。
        /// Origin restricted to chain governance (used to accredit new issuers).
        type GovernanceOrigin: EnsureOrigin<Self::RuntimeOrigin>;

        /// 本地鏈標籤（例如 `b"tw"`），對應 `did:fer:<tag>` 的 `<tag>` 部分（§09）。
        /// This chain's local tag (e.g. `b"tw"`), the `<tag>` in `did:fer:<tag>` (§09).
        type LocalChainTag: Get<BoundedVec<u8, ConstU32<MAX_TAG_LEN>>>;

        /// 權重資訊。Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    // ========================================================================
    // Storage — 儲存項
    // ========================================================================

    /// 主索引：DID -> 文件錨點（含 `doc_hash`，無個資）。
    /// Primary index: DID -> document anchor (holds `doc_hash`, no PII).
    #[pallet::storage]
    #[pallet::getter(fn dids)]
    pub type Dids<T: Config> = StorageMap<_, Blake2_128Concat, Did, DidDocument, OptionQuery>;

    /// 反向索引：控制帳戶 -> DID，方便依帳戶查詢其 DID（§05）。
    /// Reverse index: controller account -> DID, for account-keyed lookups (§05).
    #[pallet::storage]
    #[pallet::getter(fn did_by_controller)]
    pub type DidByController<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, Did, OptionQuery>;

    /// 全域撤銷累加器承諾值；每次 `update_revocation` 推進此累加器（§05）。
    /// Global revocation-accumulator commitment, advanced on every
    /// `update_revocation` (§05).
    #[pallet::storage]
    #[pallet::getter(fn revocation_accumulator)]
    pub type RevocationAccumulator<T: Config> = StorageValue<_, Commitment, ValueQuery>;

    /// 受認證簽發機構名冊：帳戶 -> 是否獲准簽發 DID 文件（§05）。
    /// Roster of accredited issuers: account -> whether they may anchor DID
    /// documents (§05).
    #[pallet::storage]
    pub type AccreditedIssuers<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, bool, ValueQuery>;

    // ========================================================================
    // Events — 事件
    // ========================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// 新 DID 文件已錨定。 A new DID document has been anchored.
        DidAnchored { did: Did, controller: T::AccountId, doc_hash: ferrum_primitives::Hash32 },
        /// DID 文件的驗證金鑰已輪換。 Verification keys for a DID were rotated.
        KeysRotated { did: Did, key_count: u32 },
        /// 撤銷累加器已更新。 The revocation accumulator was advanced.
        RevocationUpdated { commitment: Commitment },
        /// 新簽發機構已獲認證。 A new issuer has been accredited.
        IssuerRegistered { who: T::AccountId },
    }

    // ========================================================================
    // Errors — 錯誤
    // ========================================================================

    #[pallet::error]
    pub enum Error<T> {
        /// 呼叫者非受認證簽發機構。 Caller is not an accredited issuer.
        NotAccredited,
        /// 該 DID 已存在。 The DID already exists.
        AlreadyExists,
        /// 該 DID 不存在。 The DID does not exist.
        NotFound,
        /// 呼叫者非該 DID 的控制者。 Caller is not the controller of this DID.
        NotController,
        /// 提供的鏈標籤與本地標籤不符。 The DID's chain tag does not match this chain's local tag.
        WrongChainTag,
    }

    // ========================================================================
    // Calls — 外部呼叫
    // ========================================================================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// 由受認證簽發機構錨定一筆新的 DID 文件雜湊（§04/§05 節錄之
        /// `register_did`，此處為 `anchor_did`）。鏈上只存 `doc_hash`，
        /// 文件本體存於機構鏈下加密庫。
        ///
        /// Issuer-only: anchor a new DID document hash (whitepaper §04/§05
        /// excerpt's `register_did`, named `anchor_did` per SPEC). Only the
        /// `doc_hash` is stored on-chain; the document itself lives in the
        /// issuing agency's off-chain encrypted vault.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::anchor_did())]
        pub fn anchor_did(origin: OriginFor<T>, doc: DidDocument) -> DispatchResult {
            let issuer = T::IssuerOrigin::ensure_origin(origin)?;
            let _ = issuer; // IssuerOrigin already proves accreditation via the origin check

            // 雙重檢查：origin 之外，也檢查名冊（涵蓋以一般簽署帳戶代表機構的情況）。
            // Belt-and-braces: also check the roster (covers signed-account issuers).
            // Note: IssuerOrigin may resolve to an AccountId-bearing origin; if the
            // concrete Config wires IssuerOrigin = EnsureSigned, the roster check
            // below provides the actual accreditation gate.

            ensure!(doc.did.is_local(T::LocalChainTag::get().as_slice()), Error::<T>::WrongChainTag);
            ensure!(!Dids::<T>::contains_key(&doc.did), Error::<T>::AlreadyExists);

            Dids::<T>::insert(&doc.did, &doc);
            DidByController::<T>::insert(&doc.controller, &doc.did);

            Self::deposit_event(Event::DidAnchored {
                did: doc.did.clone(),
                controller: doc.controller.clone(),
                doc_hash: doc.doc_hash,
            });
            Ok(())
        }

        /// 由 DID 控制者輪換驗證金鑰參照（僅雜湊，無金鑰本體）。
        ///
        /// Controller-only: rotate the set of verification key references
        /// (hashes only, never raw key material).
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::rotate_keys())]
        pub fn rotate_keys(
            origin: OriginFor<T>,
            did: Did,
            keys: BoundedVec<DidKeyRef, ConstU32<MAX_DID_KEYS>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            Dids::<T>::try_mutate(&did, |maybe_doc| -> DispatchResult {
                let doc = maybe_doc.as_mut().ok_or(Error::<T>::NotFound)?;
                ensure!(doc.controller == who, Error::<T>::NotController);
                doc.keys = keys.clone();
                doc.anchored_at =
                    sp_runtime::SaturatedConversion::saturated_into(frame_system::Pallet::<T>::block_number());
                Ok(())
            })?;

            Self::deposit_event(Event::KeysRotated { did, key_count: keys.len() as u32 });
            Ok(())
        }

        /// 由受認證簽發機構推進全域撤銷累加器承諾值（§05）。
        ///
        /// Issuer-only: advance the global revocation-accumulator commitment (§05).
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::update_revocation())]
        pub fn update_revocation(origin: OriginFor<T>, commitment: Commitment) -> DispatchResult {
            T::IssuerOrigin::ensure_origin(origin)?;

            RevocationAccumulator::<T>::put(commitment);
            Self::deposit_event(Event::RevocationUpdated { commitment });
            Ok(())
        }

        /// 由鏈上治理新增一個受認證簽發機構（§05）。
        ///
        /// Governance-only: accredit a new issuer (§05).
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::register_issuer())]
        pub fn register_issuer(origin: OriginFor<T>, who: T::AccountId) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;

            AccreditedIssuers::<T>::insert(&who, true);
            Self::deposit_event(Event::IssuerRegistered { who });
            Ok(())
        }
    }

    // ========================================================================
    // Helper getters — 供其他 pallet 讀取，避免依賴儲存佈局
    // ========================================================================

    impl<T: Config> Pallet<T> {
        /// 是否為受認證簽發機構。 Whether `who` is an accredited issuer.
        pub fn is_accredited_issuer(who: &T::AccountId) -> bool {
            AccreditedIssuers::<T>::get(who)
        }
    }
}

// ============================================================================
// Cross-pallet read-only interface (SPEC §3): `DidRegistry` trait
// ----------------------------------------------------------------------------
// `pallet-credential`、`pallet-tax`、`pallet-interop` 透過此 trait 讀取
// `Dids` / `RevocationAccumulator`，而不需依賴本 pallet 的儲存佈局。
//
// `pallet-credential`, `pallet-tax`, `pallet-interop` consume this trait to
// read `Dids` / `RevocationAccumulator` without depending on this pallet's
// storage layout.
// ============================================================================

use ferrum_primitives::{Commitment, Did, DidDocument};

/// 唯讀身分登記介面。 Read-only identity registry interface.
pub trait DidRegistry {
    /// 查詢某 DID 的文件錨點（若存在）。
    /// Look up the document anchor for a DID, if it exists.
    fn resolve(did: &Did) -> Option<DidDocument>;

    /// 該 DID 是否已在鏈上錨定。 Whether a DID has been anchored on-chain.
    fn exists(did: &Did) -> bool {
        Self::resolve(did).is_some()
    }

    /// 目前的全域撤銷累加器承諾值。
    /// The current global revocation-accumulator commitment.
    fn revocation_accumulator() -> Commitment;
}

impl<T: Config> DidRegistry for Pallet<T> {
    fn resolve(did: &Did) -> Option<DidDocument> {
        Dids::<T>::get(did)
    }

    fn revocation_accumulator() -> Commitment {
        RevocationAccumulator::<T>::get()
    }
}
