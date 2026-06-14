//! # Ferrum 鐵鏈 — shared primitives
//!
//! `no_std` crate holding the **common types every pallet and the runtime
//! agree on**. It is the linchpin of the workspace: pallets `identity`, `tax`,
//! `credential`, `treasury`, `federation`, `interop`, the `zk` crate, the
//! `runtime` and the `node` all depend on it for account/balance aliases, the
//! `did:fer` decentralized-identifier types (whitepaper §05), tax types (§06),
//! the dual-asset / FER constants (§08), and the cross-border XSU basket +
//! federation types (§09–§11).
//!
//! **Privacy invariant (whitepaper §03/§05):** PII never enters L1–L3. The
//! types here only ever carry *commitments and hashes* on-chain — never
//! plaintext personal data. `Hash32`/`Commitment` model that explicitly.
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::vec::Vec;
use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_core::{ConstU32, RuntimeDebug};
use sp_runtime::{
    traits::{BlakeTwo256, IdentifyAccount, Verify},
    BoundedVec, MultiSignature, Perbill,
};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

// ============================================================================
// 1. Core chain aliases (shared by runtime + every pallet)
// ============================================================================

/// Signature type used across the chain. Accounts sign with sr25519; validators
/// may use ed25519 (whitepaper §04 — schnorrkel / ed25519-dalek). `MultiSignature`
/// covers both plus ecdsa.
pub type Signature = MultiSignature;

/// The public-key half of [`Signature`].
pub type AccountPublic = <Signature as Verify>::Signer;

/// Canonical account identifier (32-byte sr25519/ed25519 public key id).
pub type AccountId = <AccountPublic as IdentifyAccount>::AccountId;

/// Index of a transaction in the chain (a.k.a. account nonce).
pub type Nonce = u32;

/// Balance of an account. Used for **FER** (the non-speculative network token,
/// §08) and as the integer base unit for fiat-pegged amounts.
pub type Balance = u128;

/// On-chain block number.
pub type BlockNumber = u32;

/// A timestamp in **milliseconds** since the Unix epoch (matches
/// `pallet-timestamp::Moment`).
pub type Moment = u64;

/// The hashing algorithm of the state trie and all commitments (BLAKE2b-256,
/// whitepaper §04).
pub type Hashing = BlakeTwo256;

/// Output of [`Hashing`]: a 32-byte hash. The **only** representation of
/// personal data that is permitted on-chain.
pub type Hash = sp_core::H256;

/// A raw 32-byte commitment / hash digest (e.g. a DID-document hash, an
/// e-invoice hash, a revocation-accumulator commitment).
pub type Hash32 = [u8; 32];

/// Semantic alias making call sites self-documenting: a cryptographic
/// commitment that hides the underlying (off-chain) PII.
pub type Commitment = Hash32;

/// A one-time nullifier emitted by a selective-disclosure proof (§05 Flow B).
pub type Nullifier = Hash32;

// ============================================================================
// 2. Bound constants (used in BoundedVec capacities across pallets)
// ============================================================================

/// Max length of a `did:fer` method-specific identifier string, in bytes.
pub const MAX_DID_LEN: u32 = 128;
/// Max number of verification/service entries referenced by a DID document.
pub const MAX_DID_KEYS: u32 = 16;
/// Max length of an ISO-style scheme tag (e.g. `"did:fer:tw"` country part).
pub const MAX_TAG_LEN: u32 = 16;
/// Max members in the federation treaty council / XSU basket.
pub const MAX_FEDERATION_MEMBERS: u32 = 64;
/// Max metadata blob length (URIs, encrypted-vault pointers) in bytes.
pub const MAX_META_LEN: u32 = 256;

/// Bounded byte blob for short metadata pointers (never PII — only URIs/hashes).
pub type MetaBlob = BoundedVec<u8, ConstU32<MAX_META_LEN>>;

// ============================================================================
// 3. Token / fiscal constants (whitepaper §07 consensus, §08 token model)
// ============================================================================

/// FER uses 12 decimal places (Substrate default planck granularity).
pub const FER_DECIMALS: u32 = 12;

/// One whole **FER** in its smallest unit. Referenced by runtime consensus
/// constants such as `MinValidatorBond = 250_000 * FER` (whitepaper §07,
/// `runtime/src/consensus.rs`).
pub const FER: Balance = 1_000_000_000_000; // 10^12

/// Convenience: minimum validator bond from the whitepaper consensus excerpt.
pub const MIN_VALIDATOR_BOND: Balance = 250_000 * FER;

/// Block authoring slot duration in milliseconds (whitepaper §07: 3-second slots).
pub const SLOT_DURATION_MS: Moment = 3_000;

/// Cap on accredited institutional validator nodes (whitepaper §07).
pub const MAX_AUTHORITIES: u32 = 100;

// ============================================================================
// 4. Identity layer (whitepaper §05) — did:fer
// ============================================================================

/// A `did:fer` decentralized identifier, stored as the method-specific id only.
///
/// On-chain we keep the identifier and a *source-chain tag* (so `did:fer:tw`
/// vs `did:fer:jp` resolve cross-chain, §09), plus the **hash** of the DID
/// document — never the document itself.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Encode, Decode, MaxEncodedLen, TypeInfo, RuntimeDebug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Did {
    /// Source-chain / country tag, e.g. `b"tw"`, `b"jp"` (the `did:fer:<tag>` part).
    pub chain_tag: BoundedVec<u8, ConstU32<MAX_TAG_LEN>>,
    /// Method-specific identifier bytes (an encoded public-key id / UUID).
    pub id: BoundedVec<u8, ConstU32<MAX_DID_LEN>>,
}

impl Did {
    /// True if this DID was issued on the local chain carrying `local_tag`.
    pub fn is_local(&self, local_tag: &[u8]) -> bool {
        self.chain_tag.as_slice() == local_tag
    }
}

/// The kind of cryptographic key referenced by a DID document entry.
#[derive(Clone, Copy, Eq, PartialEq, Encode, Decode, MaxEncodedLen, TypeInfo, RuntimeDebug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum KeyKind {
    /// sr25519 (schnorrkel) — the citizen device key (§05 Flow A step 2).
    Sr25519,
    /// ed25519 — institutional / validator keys.
    Ed25519,
    /// BLS12-381 — used by BBS+ / Groth16 issuer commitments (§05).
    Bls12_381,
}

/// A single key reference inside a DID document (only its hash/id is on-chain).
#[derive(Clone, Eq, PartialEq, Encode, Decode, MaxEncodedLen, TypeInfo, RuntimeDebug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct DidKeyRef {
    pub kind: KeyKind,
    /// Hash of the public key material (keys rotate without changing the DID).
    pub key_hash: Hash32,
}

/// On-chain anchor for a DID. **No PII** — only the document hash, the
/// controller account, key references, and a revocation-status commitment.
/// This mirrors `pallets/identity/src/lib.rs` (whitepaper §04 excerpt: only
/// `doc_hash` is stored on-chain).
#[derive(Clone, Eq, PartialEq, Encode, Decode, MaxEncodedLen, TypeInfo, RuntimeDebug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct DidDocument {
    /// The DID this document anchors.
    pub did: Did,
    /// Account that controls (can update/rotate) this DID.
    pub controller: AccountId,
    /// BLAKE2b hash of the off-chain DID document (the `doc_hash`).
    pub doc_hash: Hash32,
    /// Referenced verification keys (hashes only).
    pub keys: BoundedVec<DidKeyRef, ConstU32<MAX_DID_KEYS>>,
    /// Commitment to the revocation-status accumulator for this subject (§05).
    pub revocation_commitment: Commitment,
    /// Block at which this anchor was created/last updated.
    pub anchored_at: BlockNumber,
}

// ============================================================================
// 5. Credential layer (whitepaper §05) — Verifiable Credentials
// ============================================================================

/// The class of claim asserted by a Verifiable Credential. The credential
/// *content* lives off-chain; only its type + hash + issuer are anchored.
#[derive(Clone, Copy, Eq, PartialEq, Encode, Decode, MaxEncodedLen, TypeInfo, RuntimeDebug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum CredentialKind {
    Nationality,
    Age,
    Residence,
    /// Tax-status / tax-residency credential (binds tax identity to the DID, §06).
    TaxStatus,
    /// Open-ended issuer-defined credential class.
    Other,
}

/// Lifecycle status of an anchored credential.
#[derive(Clone, Copy, Eq, PartialEq, Encode, Decode, MaxEncodedLen, TypeInfo, RuntimeDebug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum CredentialStatus {
    Active,
    Suspended,
    Revoked,
    Expired,
}

/// On-chain anchor of a Verifiable Credential. Holds **no** claim values, only
/// the issuer-signed hash and metadata needed for offline + on-chain checks.
#[derive(Clone, Eq, PartialEq, Encode, Decode, MaxEncodedLen, TypeInfo, RuntimeDebug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CredentialAnchor {
    /// Subject DID this credential is about.
    pub subject: Did,
    /// Account of the accredited issuer (civil registry / KYC institution).
    pub issuer: AccountId,
    pub kind: CredentialKind,
    /// Hash of the issuer-signed credential payload (offline-verifiable).
    pub payload_hash: Hash32,
    pub status: CredentialStatus,
    /// Optional expiry (milliseconds since epoch); `None` = no expiry.
    pub expires_at: Option<Moment>,
}

// ============================================================================
// 6. Tax layer (whitepaper §06)
// ============================================================================

/// ISO-4217-ish fiat currency code for **obligations** (always fiat-denominated,
/// never the volatile network token). Stored as 3 ASCII bytes, e.g. `b"TWD"`.
pub type FiatCurrency = [u8; 3];

/// A fiat amount denominated in [`FiatCurrency`], in the currency's minor unit
/// (e.g. cents). Tax obligations are ALWAYS fiat-denominated (whitepaper §06).
#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Encode, Decode, MaxEncodedLen, TypeInfo, RuntimeDebug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct FiatAmount {
    pub currency: FiatCurrency,
    /// Amount in minor units (e.g. cents). `u128` to be future-proof.
    pub minor_units: u128,
}

/// A tax bracket reference — taxpayers prove "income in bracket X" via ZK
/// without revealing the exact amount (whitepaper §06 ZK bracket proofs).
#[derive(Clone, Copy, Eq, PartialEq, Encode, Decode, MaxEncodedLen, TypeInfo, RuntimeDebug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TaxBracket {
    /// Bracket index (0-based, ascending).
    pub index: u8,
    /// Applicable rate for this bracket.
    pub rate: Perbill,
}

/// Category of taxable event / withholding (whitepaper §06 programmable withholding).
#[derive(Clone, Copy, Eq, PartialEq, Encode, Decode, MaxEncodedLen, TypeInfo, RuntimeDebug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum TaxKind {
    Income,
    Wage,
    Interest,
    /// Value-added / goods-and-services tax (§09 cross-border VAT/GST).
    ValueAdded,
    Withholding,
    Other,
}

/// An e-invoice anchor: only the invoice **hash** is on-chain; line items stay
/// encrypted off-chain (whitepaper §06 e-invoice anchoring).
#[derive(Clone, Eq, PartialEq, Encode, Decode, MaxEncodedLen, TypeInfo, RuntimeDebug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct InvoiceAnchor {
    /// BLAKE2b hash of the full (off-chain) e-invoice.
    pub invoice_hash: Hash32,
    /// Issuing merchant/agency account.
    pub issuer: AccountId,
    /// Tax category for this invoice.
    pub kind: TaxKind,
    /// Anchoring time (ms since epoch).
    pub anchored_at: Moment,
}

/// A tax obligation / filing record. The amount due is fiat-denominated; the
/// detail is a commitment, auditable only with an authorized viewing key (§06).
#[derive(Clone, Eq, PartialEq, Encode, Decode, MaxEncodedLen, TypeInfo, RuntimeDebug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TaxObligation {
    /// Subject DID (tax identity binds to the DID, §06).
    pub subject: Did,
    pub kind: TaxKind,
    /// Amount due, always in fiat.
    pub amount_due: FiatAmount,
    /// Commitment to the encrypted return detail (no PII on-chain).
    pub detail_commitment: Commitment,
    /// Whether the obligation has been settled (paid in eTWD CBDC).
    pub settled: bool,
}

// ============================================================================
// 7. Federation + XSU cross-border layer (whitepaper §09–§11)
// ============================================================================

/// ISO 3166-style country code (2 ASCII bytes, e.g. `b"TW"`, `b"JP"`).
pub type CountryId = [u8; 2];

/// Identifier of a federation (treaty union of sovereign chains, §09).
pub type FederationId = u32;

/// Identifier of a federation **member** seat (one seat per nation, §11.1).
/// Equal to the member's `CountryId` lifted into a comparable scalar key.
pub type MemberId = CountryId;

/// Proposal identifier within a federation governance round (§11.4).
pub type ProposalId = u64;

/// A CBDC / fiat-stablecoin code in the XSU basket (e.g. `b"USD"`, `b"EUR"`).
pub type CbdcCode = FiatCurrency;

/// A single basket entry: a member CBDC and its fixed weight (whitepaper §10).
#[derive(Clone, Copy, Eq, PartialEq, Encode, Decode, MaxEncodedLen, TypeInfo, RuntimeDebug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct BasketEntry {
    pub cbdc: CbdcCode,
    /// Fixed weight of this CBDC in the XSU basket (must sum to 100% across the basket).
    pub weight: Perbill,
}

/// The **XSU** — a neutral, fully-reserved synthetic unit of account defined as
/// a fixed-weight basket of member CBDCs (whitepaper §10, "digital SDR").
#[derive(Clone, Eq, PartialEq, Encode, Decode, MaxEncodedLen, TypeInfo, RuntimeDebug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct XsuBasket {
    /// Fixed-weight components (whitepaper illustrative: eUSD 40 / eEUR 28 / …).
    pub entries: BoundedVec<BasketEntry, ConstU32<MAX_FEDERATION_MEMBERS>>,
    /// Monotonic version bumped on every reweighting (§11.3).
    pub version: u32,
}

impl XsuBasket {
    /// Sum of all component weights — should equal `Perbill::one()` (100%).
    pub fn total_weight(&self) -> Perbill {
        self.entries
            .iter()
            .fold(Perbill::zero(), |a, e| a.saturating_add(e.weight))
    }

    /// `true` if the basket weights sum to exactly 100%.
    pub fn is_balanced(&self) -> bool {
        self.total_weight() == Perbill::one()
    }
}

/// An amount denominated in **XSU** (cross-border obligations are priced here, §10).
/// Integer minor units with [`XSU_DECIMALS`] precision.
#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Encode, Decode, MaxEncodedLen, TypeInfo, RuntimeDebug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct XsuAmount(pub u128);

/// Decimal precision of [`XsuAmount`].
pub const XSU_DECIMALS: u32 = 6;

/// A vote cast by a council member (whitepaper §11.2 dual-majority).
///
/// **Contract:** `pallets/federation/src/voting.rs::passes_dual_majority`
/// compares against `Vote::Aye` exactly as shown in the whitepaper excerpt.
#[derive(Clone, Copy, Eq, PartialEq, Encode, Decode, MaxEncodedLen, TypeInfo, RuntimeDebug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Vote {
    Aye,
    Nay,
    Abstain,
}

/// Governance domains, each with its own threshold + timelock (§11.2 table).
#[derive(Clone, Copy, Eq, PartialEq, Encode, Decode, MaxEncodedLen, TypeInfo, RuntimeDebug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum GovernanceDomain {
    /// Parameter change (fees, slots): dual-majority ⅔, 7-day timelock.
    Parameter,
    /// Member admission / removal: dual-majority ¾, 30-day timelock.
    Membership,
    /// Basket reweighting: dual-majority ⅔ + secretariat report.
    Reweighting,
    /// Basket redefinition / treaty amendment: near-unanimity ≥ 85%.
    Constitutional,
    /// Emergency security upgrade: tech committee + post-ratify within 14 days.
    Emergency,
    /// Dispute arbitration / member suspension: dual-majority ¾ (party recused).
    Dispute,
}

/// A concrete federation-level action subject to governance (whitepaper §11.4
/// `FederationAction`, consumed by `pallet-federation::propose`).
#[derive(Clone, Eq, PartialEq, Encode, Decode, MaxEncodedLen, TypeInfo, RuntimeDebug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum FederationAction {
    /// Adjust a fee or consensus parameter.
    SetParameter { key: BoundedVec<u8, ConstU32<MAX_TAG_LEN>>, value: u128 },
    /// Admit a new member nation.
    AdmitMember { member: MemberId },
    /// Remove / expel a member nation.
    RemoveMember { member: MemberId },
    /// Replace the XSU basket weights with a new balanced basket.
    Reweight { basket: XsuBasket },
    /// Suspend a member's authoring/clearing rights (claim on reserves preserved).
    SuspendMember { member: MemberId },
    /// Trigger a forkless runtime upgrade (WASM code hash anchored off-chain blob).
    RuntimeUpgrade { code_hash: Hash32 },
}

impl FederationAction {
    /// The governance domain this action falls under (drives threshold + timelock).
    pub fn domain(&self) -> GovernanceDomain {
        match self {
            FederationAction::SetParameter { .. } => GovernanceDomain::Parameter,
            FederationAction::AdmitMember { .. } | FederationAction::RemoveMember { .. } => {
                GovernanceDomain::Membership
            }
            FederationAction::Reweight { .. } => GovernanceDomain::Reweighting,
            FederationAction::SuspendMember { .. } => GovernanceDomain::Dispute,
            FederationAction::RuntimeUpgrade { .. } => GovernanceDomain::Constitutional,
        }
    }
}

/// Helper: the dual-majority threshold (numerator/denominator) for a domain,
/// per the whitepaper §11.2 table. Returned as a `Perbill`.
pub fn domain_threshold(domain: GovernanceDomain) -> Perbill {
    match domain {
        GovernanceDomain::Parameter | GovernanceDomain::Reweighting => {
            Perbill::from_rational(2u32, 3u32)
        }
        GovernanceDomain::Membership | GovernanceDomain::Dispute => {
            Perbill::from_rational(3u32, 4u32)
        }
        GovernanceDomain::Constitutional => Perbill::from_percent(85),
        GovernanceDomain::Emergency => Perbill::from_percent(0), // tech committee path
    }
}

// ============================================================================
// 8. Interop / bridge layer (whitepaper §09, §12 Flow E)
// ============================================================================

/// A registered accredited issuer in the cross-chain **trust registry** (§09).
#[derive(Clone, Eq, PartialEq, Encode, Decode, MaxEncodedLen, TypeInfo, RuntimeDebug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TrustRegistryEntry {
    /// Country whose authority recognizes this issuer.
    pub country: CountryId,
    /// Hash of the issuer's recognized public key.
    pub issuer_key_hash: Hash32,
    /// Recognition scope (encoded treaty scope tag — never PII).
    pub scope: BoundedVec<u8, ConstU32<MAX_TAG_LEN>>,
    pub active: bool,
}

/// Status of an inbound/outbound cross-consensus (XCM-style) message (§09).
#[derive(Clone, Copy, Eq, PartialEq, Encode, Decode, MaxEncodedLen, TypeInfo, RuntimeDebug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum XcmStatus {
    Pending,
    /// Verified against the source chain's GRANDPA finality proof.
    FinalityVerified,
    Accepted,
    Rejected,
}

/// A cross-border settlement instruction, priced in XSU and netted multilaterally
/// before settling net positions in CBDCs (whitepaper §10–§11, §12 Flow E step 4).
#[derive(Clone, Eq, PartialEq, Encode, Decode, MaxEncodedLen, TypeInfo, RuntimeDebug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ClearingInstruction {
    pub from: CountryId,
    pub to: CountryId,
    /// Amount priced in the neutral XSU unit.
    pub amount: XsuAmount,
    /// Commitment to the underlying obligation detail (no PII crosses borders).
    pub detail_commitment: Commitment,
    pub status: XcmStatus,
}

// ============================================================================
// 9. ZK public-input shapes (consumed by crates/zk + pallets that verify)
// ============================================================================

/// The public inputs to the age-threshold proof, in the order the verifier
/// expects (whitepaper §05 excerpt: `[issuer_commitment, threshold, nullifier]`).
/// The `zk` crate maps these into field elements; pallets pass them opaquely.
#[derive(Clone, Eq, PartialEq, Encode, Decode, MaxEncodedLen, TypeInfo, RuntimeDebug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct AgeProofPublicInputs {
    /// Commitment to the issuer of the underlying credential.
    pub issuer_commitment: Commitment,
    /// The age threshold being proven (e.g. 18).
    pub threshold: u32,
    /// One-time nullifier preventing replay.
    pub nullifier: Nullifier,
}

/// Opaque, length-bounded serialized ZK proof bytes carried by extrinsics.
/// The `zk` crate owns (de)serialization into arkworks `Proof<Bls12_381>`.
pub type ProofBytes = BoundedVec<u8, ConstU32<2048>>;

/// Opaque serialized verifying key bytes (arkworks `PreparedVerifyingKey`).
pub type VerifyingKeyBytes = Vec<u8>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fer_unit_matches_decimals() {
        assert_eq!(FER, 10u128.pow(FER_DECIMALS));
        assert_eq!(MIN_VALIDATOR_BOND, 250_000 * FER);
    }

    #[test]
    fn balanced_basket_sums_to_one() {
        let mut entries = BoundedVec::new();
        entries.try_push(BasketEntry { cbdc: *b"USD", weight: Perbill::from_percent(40) }).unwrap();
        entries.try_push(BasketEntry { cbdc: *b"EUR", weight: Perbill::from_percent(28) }).unwrap();
        entries.try_push(BasketEntry { cbdc: *b"CNY", weight: Perbill::from_percent(12) }).unwrap();
        entries.try_push(BasketEntry { cbdc: *b"JPY", weight: Perbill::from_percent(8) }).unwrap();
        entries.try_push(BasketEntry { cbdc: *b"OTH", weight: Perbill::from_percent(12) }).unwrap();
        let basket = XsuBasket { entries, version: 1 };
        assert!(basket.is_balanced());
    }

    #[test]
    fn domain_thresholds_match_whitepaper() {
        assert_eq!(domain_threshold(GovernanceDomain::Parameter), Perbill::from_rational(2u32, 3u32));
        assert_eq!(domain_threshold(GovernanceDomain::Membership), Perbill::from_rational(3u32, 4u32));
        assert_eq!(domain_threshold(GovernanceDomain::Constitutional), Perbill::from_percent(85));
    }
}
