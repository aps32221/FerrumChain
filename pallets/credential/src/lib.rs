//! # pallet-credential — 可驗證憑證 (Verifiable Credentials) — 鐵鏈 Ferrum (§05)
//!
//! Anchors issuer-signed Verifiable Credential (VC) hashes and lifecycle
//! status on-chain, and logs replay-protected selective-disclosure
//! presentations. **No claim values are ever stored on-chain** — only
//! [`ferrum_primitives::Hash32`] payload hashes, [`ferrum_primitives::Commitment`]
//! commitments, and [`ferrum_primitives::Nullifier`] nullifiers (whitepaper
//! §03/§05 privacy invariant).
//!
//! 本模組僅在鏈上錨定憑證簽章雜湊與生命週期狀態，並記錄具重放保護的選擇性
//! 揭露憑證展示。鏈上**絕不**儲存任何聲明明文，僅保存雜湊與承諾值
//! （白皮書 §03/§05 隱私不變式）。
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
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    use ferrum_primitives::{
        Commitment, CredentialAnchor, CredentialStatus, Did, Hash32, Nullifier,
    };

    /// Max number of credential hashes indexed per subject DID.
    /// 每個主體 DID 最多索引的憑證雜湊數量。
    pub const MAX_CREDENTIALS_PER_SUBJECT: u32 = 64;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// 事件型別 / The overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// 受認可發證機構之來源檢查 / Origin allowed to issue, revoke, and update
        /// credential status (accredited issuer institutions, §05).
        type IssuerOrigin: EnsureOrigin<Self::RuntimeOrigin>;

        /// 權重資訊 / Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    // ========================================================================
    // Storage 儲存項
    // ========================================================================

    /// 憑證錨點：以憑證簽章內容雜湊 (payload_hash) 為鍵。
    /// Credential anchors keyed by the issuer-signed payload hash.
    #[pallet::storage]
    pub type Credentials<T: Config> =
        StorageMap<_, Blake2_128Concat, Hash32, CredentialAnchor, OptionQuery>;

    /// 依主體 DID 索引的憑證雜湊列表（上限 64 筆）。
    /// Index of credential payload hashes by subject DID (bounded to 64).
    #[pallet::storage]
    pub type BySubject<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        Did,
        BoundedVec<Hash32, ConstU32<MAX_CREDENTIALS_PER_SUBJECT>>,
        ValueQuery,
    >;

    /// 一次性、防重放的選擇性揭露展示紀錄：nullifier -> 揭露承諾值。
    /// One-time, replay-protected selective-disclosure presentation log:
    /// nullifier -> disclosure commitment.
    #[pallet::storage]
    pub type Presentations<T: Config> =
        StorageMap<_, Blake2_128Concat, Nullifier, Commitment, OptionQuery>;

    // ========================================================================
    // Events 事件
    // ========================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// 已核發新憑證 / A new credential was issued and anchored.
        CredentialIssued { subject: Did, issuer: T::AccountId, payload_hash: Hash32 },
        /// 憑證已被撤銷 / A credential was revoked by its issuer.
        CredentialRevoked { payload_hash: Hash32 },
        /// 憑證狀態已更新 / A credential's lifecycle status was updated.
        CredentialStatusUpdated { payload_hash: Hash32, status: CredentialStatus },
        /// 選擇性揭露展示已記錄 / A selective-disclosure presentation was logged.
        PresentationLogged { nullifier: Nullifier, commitment: Commitment },
    }

    // ========================================================================
    // Errors 錯誤
    // ========================================================================

    #[pallet::error]
    pub enum Error<T> {
        /// 該憑證雜湊已存在 / A credential with this payload hash already exists.
        CredentialAlreadyExists,
        /// 找不到該憑證 / The referenced credential does not exist.
        CredentialNotFound,
        /// 主體索引已達上限 / The subject's credential index is full.
        TooManyCredentialsForSubject,
        /// 該 nullifier 已被使用（重放） / This nullifier has already been used (replay).
        PresentationAlreadyLogged,
    }

    // ========================================================================
    // Calls 外部呼叫
    // ========================================================================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// 核發憑證錨點（僅限受認可發證機構）。
        /// Issue (anchor) a new credential. Issuer-only.
        ///
        /// Stores only the [`CredentialAnchor`] — issuer-signed `payload_hash`,
        /// subject DID, kind, status and optional expiry. No claim content.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::issue())]
        pub fn issue(origin: OriginFor<T>, anchor: CredentialAnchor) -> DispatchResult {
            T::IssuerOrigin::ensure_origin(origin)?;

            ensure!(
                !Credentials::<T>::contains_key(anchor.payload_hash),
                Error::<T>::CredentialAlreadyExists
            );

            let subject = anchor.subject.clone();
            let issuer = anchor.issuer.clone();
            let payload_hash = anchor.payload_hash;

            BySubject::<T>::try_mutate(&subject, |hashes| -> DispatchResult {
                hashes
                    .try_push(payload_hash)
                    .map_err(|_| Error::<T>::TooManyCredentialsForSubject)?;
                Ok(())
            })?;

            Credentials::<T>::insert(payload_hash, anchor);

            Self::deposit_event(Event::CredentialIssued { subject, issuer, payload_hash });
            Ok(())
        }

        /// 撤銷憑證（僅限發證機構）。狀態被設為 `Revoked`。
        /// Revoke a credential. Issuer-only. Sets status to `Revoked`.
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::revoke())]
        pub fn revoke(origin: OriginFor<T>, payload_hash: Hash32) -> DispatchResult {
            T::IssuerOrigin::ensure_origin(origin)?;

            Credentials::<T>::try_mutate(payload_hash, |maybe_anchor| -> DispatchResult {
                let anchor = maybe_anchor.as_mut().ok_or(Error::<T>::CredentialNotFound)?;
                anchor.status = CredentialStatus::Revoked;
                Ok(())
            })?;

            Self::deposit_event(Event::CredentialRevoked { payload_hash });
            Ok(())
        }

        /// 更新憑證生命週期狀態（僅限發證機構）。
        /// Update a credential's lifecycle status. Issuer-only.
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::set_status())]
        pub fn set_status(
            origin: OriginFor<T>,
            payload_hash: Hash32,
            status: CredentialStatus,
        ) -> DispatchResult {
            T::IssuerOrigin::ensure_origin(origin)?;

            Credentials::<T>::try_mutate(payload_hash, |maybe_anchor| -> DispatchResult {
                let anchor = maybe_anchor.as_mut().ok_or(Error::<T>::CredentialNotFound)?;
                anchor.status = status;
                Ok(())
            })?;

            Self::deposit_event(Event::CredentialStatusUpdated { payload_hash, status });
            Ok(())
        }

        /// 記錄一次選擇性揭露的憑證展示（防重放，§05 Flow B）。
        /// Log a one-time selective-disclosure presentation (replay-protected,
        /// §05 Flow B). Any signed origin may submit — the `nullifier` itself
        /// guarantees one-time use.
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::log_presentation())]
        pub fn log_presentation(
            origin: OriginFor<T>,
            nullifier: Nullifier,
            commitment: Commitment,
        ) -> DispatchResult {
            ensure_signed(origin)?;

            ensure!(
                !Presentations::<T>::contains_key(nullifier),
                Error::<T>::PresentationAlreadyLogged
            );

            Presentations::<T>::insert(nullifier, commitment);

            Self::deposit_event(Event::PresentationLogged { nullifier, commitment });
            Ok(())
        }
    }
}
