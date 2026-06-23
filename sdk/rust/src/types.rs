//! Friendly input structs and `scale_value::Value` builders for the Ferrum
//! SCALE types.
//!
//! The Ferrum runtime ships full SCALE-info metadata (V15), so subxt encodes
//! these `Value`s into the correct on-chain types dynamically — no codegen step
//! and no committed `metadata.scale` required. Single-field newtypes (`Perbill`,
//! `XsuAmount`) are fed a bare integer; scale-value's encoder wraps it.

use subxt::ext::scale_value::Value;
use subxt::utils::AccountId32;

/// A 32-byte commitment/hash/nullifier.
pub type Bytes32 = [u8; 32];

fn bytes_value(b: &[u8]) -> Value {
    Value::from_bytes(b)
}

/// Encode an account id (32 bytes) as a Value for a `T::AccountId` arg.
pub fn account(a: &AccountId32) -> Value {
    bytes_value(&a.0)
}

/// Encode a 32-byte field.
pub fn h32(b: &Bytes32) -> Value {
    bytes_value(b)
}

/// Encode an arbitrary byte blob (proof / vk / finality proof / Bytes).
pub fn blob(b: &[u8]) -> Value {
    bytes_value(b)
}

/// Encode a short ASCII tag into a `BoundedVec<u8>` value.
pub fn ascii(s: &str) -> Value {
    bytes_value(s.as_bytes())
}

/// Fraction of one (0..=1) -> `Perbill` parts-per-billion.
pub fn perbill(frac: f64) -> Value {
    assert!((0.0..=1.0).contains(&frac), "perbill fraction must be within [0,1]");
    Value::u128((frac * 1_000_000_000.0).round() as u128)
}

fn u(n: u128) -> Value {
    Value::u128(n)
}

fn variant(name: &str) -> Value {
    Value::unnamed_variant(name, std::iter::empty::<Value>())
}

// ---- composite Ferrum types -------------------------------------------------

#[derive(Clone)]
pub struct Did {
    pub chain_tag: String,
    /// ASCII identifier (encoded as bytes) — or pass raw bytes via [`Did::from_bytes`].
    pub id: Vec<u8>,
}

impl Did {
    pub fn new(chain_tag: &str, id: &str) -> Self {
        Self { chain_tag: chain_tag.into(), id: id.as_bytes().to_vec() }
    }
    pub fn from_bytes(chain_tag: &str, id: &[u8]) -> Self {
        Self { chain_tag: chain_tag.into(), id: id.to_vec() }
    }
    pub fn to_value(&self) -> Value {
        Value::named_composite([
            ("chain_tag".into(), ascii(&self.chain_tag)),
            ("id".into(), bytes_value(&self.id)),
        ])
    }
}

pub struct DidKeyRef {
    /// "Sr25519" | "Ed25519" | "Bls12_381"
    pub kind: String,
    pub key_hash: Bytes32,
}

impl DidKeyRef {
    pub fn to_value(&self) -> Value {
        Value::named_composite([
            ("kind".into(), variant(&self.kind)),
            ("key_hash".into(), h32(&self.key_hash)),
        ])
    }
}

pub struct DidDocument {
    pub did: Did,
    pub controller: AccountId32,
    pub doc_hash: Bytes32,
    pub keys: Vec<DidKeyRef>,
    pub revocation_commitment: Bytes32,
    pub anchored_at: u32,
}

impl DidDocument {
    pub fn to_value(&self) -> Value {
        Value::named_composite([
            ("did".into(), self.did.to_value()),
            ("controller".into(), account(&self.controller)),
            ("doc_hash".into(), h32(&self.doc_hash)),
            ("keys".into(), Value::unnamed_composite(self.keys.iter().map(DidKeyRef::to_value))),
            ("revocation_commitment".into(), h32(&self.revocation_commitment)),
            ("anchored_at".into(), u(self.anchored_at as u128)),
        ])
    }
}

pub struct FiatAmount {
    /// 3-letter currency, e.g. "TWD".
    pub currency: String,
    pub minor_units: u128,
}

impl FiatAmount {
    pub fn to_value(&self) -> Value {
        assert_eq!(self.currency.len(), 3, "currency must be 3 ASCII bytes");
        Value::named_composite([
            ("currency".into(), bytes_value(self.currency.as_bytes())),
            ("minor_units".into(), u(self.minor_units)),
        ])
    }
}

pub struct CredentialAnchor {
    pub subject: Did,
    pub issuer: AccountId32,
    /// CredentialKind variant name.
    pub kind: String,
    pub payload_hash: Bytes32,
    /// CredentialStatus variant name.
    pub status: String,
    pub expires_at: Option<u64>,
}

impl CredentialAnchor {
    pub fn to_value(&self) -> Value {
        let expires = match self.expires_at {
            Some(v) => Value::unnamed_variant("Some", [u(v as u128)]),
            None => variant("None"),
        };
        Value::named_composite([
            ("subject".into(), self.subject.to_value()),
            ("issuer".into(), account(&self.issuer)),
            ("kind".into(), variant(&self.kind)),
            ("payload_hash".into(), h32(&self.payload_hash)),
            ("status".into(), variant(&self.status)),
            ("expires_at".into(), expires),
        ])
    }
}

pub struct TaxBracket {
    pub index: u8,
    pub rate: f64,
}

impl TaxBracket {
    pub fn to_value(&self) -> Value {
        Value::named_composite([
            ("index".into(), u(self.index as u128)),
            ("rate".into(), perbill(self.rate)),
        ])
    }
}

pub struct InvoiceAnchor {
    pub invoice_hash: Bytes32,
    pub issuer: AccountId32,
    pub kind: String,
    pub anchored_at: u64,
}

impl InvoiceAnchor {
    pub fn to_value(&self) -> Value {
        Value::named_composite([
            ("invoice_hash".into(), h32(&self.invoice_hash)),
            ("issuer".into(), account(&self.issuer)),
            ("kind".into(), variant(&self.kind)),
            ("anchored_at".into(), u(self.anchored_at as u128)),
        ])
    }
}

pub struct TaxObligation {
    pub subject: Did,
    pub kind: String,
    pub amount_due: FiatAmount,
    pub detail_commitment: Bytes32,
    pub settled: bool,
}

impl TaxObligation {
    pub fn to_value(&self) -> Value {
        Value::named_composite([
            ("subject".into(), self.subject.to_value()),
            ("kind".into(), variant(&self.kind)),
            ("amount_due".into(), self.amount_due.to_value()),
            ("detail_commitment".into(), h32(&self.detail_commitment)),
            ("settled".into(), Value::bool(self.settled)),
        ])
    }
}

pub struct AgeProofPublicInputs {
    pub issuer_commitment: Bytes32,
    pub threshold: u32,
    pub nullifier: Bytes32,
}

impl AgeProofPublicInputs {
    pub fn to_value(&self) -> Value {
        Value::named_composite([
            ("issuer_commitment".into(), h32(&self.issuer_commitment)),
            ("threshold".into(), u(self.threshold as u128)),
            ("nullifier".into(), h32(&self.nullifier)),
        ])
    }
}

pub struct BasketEntry {
    pub cbdc: String,
    pub weight: f64,
}

pub struct XsuBasket {
    pub entries: Vec<BasketEntry>,
    pub version: u32,
}

impl XsuBasket {
    pub fn to_value(&self) -> Value {
        let entries = self.entries.iter().map(|e| {
            assert_eq!(e.cbdc.len(), 3, "cbdc must be 3 ASCII bytes");
            Value::named_composite([
                ("cbdc".into(), bytes_value(e.cbdc.as_bytes())),
                ("weight".into(), perbill(e.weight)),
            ])
        });
        Value::named_composite([
            ("entries".into(), Value::unnamed_composite(entries)),
            ("version".into(), u(self.version as u128)),
        ])
    }
}

pub enum FederationAction {
    SetParameter { key: String, value: u128 },
    AdmitMember { member: String },
    RemoveMember { member: String },
    Reweight { basket: XsuBasket },
    SuspendMember { member: String },
    RuntimeUpgrade { code_hash: Bytes32 },
}

fn country(c: &str) -> Value {
    assert_eq!(c.len(), 2, "country must be 2 ASCII bytes");
    bytes_value(c.as_bytes())
}

impl FederationAction {
    pub fn to_value(&self) -> Value {
        match self {
            FederationAction::SetParameter { key, value } => Value::named_variant(
                "SetParameter",
                [("key".into(), ascii(key)), ("value".into(), u(*value))],
            ),
            FederationAction::AdmitMember { member } => {
                Value::named_variant("AdmitMember", [("member".into(), country(member))])
            }
            FederationAction::RemoveMember { member } => {
                Value::named_variant("RemoveMember", [("member".into(), country(member))])
            }
            FederationAction::SuspendMember { member } => {
                Value::named_variant("SuspendMember", [("member".into(), country(member))])
            }
            FederationAction::Reweight { basket } => {
                Value::named_variant("Reweight", [("basket".into(), basket.to_value())])
            }
            FederationAction::RuntimeUpgrade { code_hash } => {
                Value::named_variant("RuntimeUpgrade", [("code_hash".into(), h32(code_hash))])
            }
        }
    }
}

pub struct TrustRegistryEntry {
    pub country: String,
    pub issuer_key_hash: Bytes32,
    pub scope: String,
    pub active: bool,
}

impl TrustRegistryEntry {
    pub fn to_value(&self) -> Value {
        Value::named_composite([
            ("country".into(), country(&self.country)),
            ("issuer_key_hash".into(), h32(&self.issuer_key_hash)),
            ("scope".into(), ascii(&self.scope)),
            ("active".into(), Value::bool(self.active)),
        ])
    }
}

pub struct ClearingInstruction {
    pub from: String,
    pub to: String,
    pub amount: u128,
    pub detail_commitment: Bytes32,
    /// XcmStatus variant name; defaults to "Pending".
    pub status: Option<String>,
}

impl ClearingInstruction {
    pub fn to_value(&self) -> Value {
        let status = self.status.clone().unwrap_or_else(|| "Pending".into());
        Value::named_composite([
            ("from".into(), country(&self.from)),
            ("to".into(), country(&self.to)),
            ("amount".into(), u(self.amount)),
            ("detail_commitment".into(), h32(&self.detail_commitment)),
            ("status".into(), variant(&status)),
        ])
    }
}

pub struct TaxTreaty {
    pub withholding_cap: f64,
    /// CreditMethod variant name: "Credit" | "Exemption".
    pub method: String,
    pub active: bool,
}

impl TaxTreaty {
    pub fn to_value(&self) -> Value {
        Value::named_composite([
            ("withholding_cap".into(), perbill(self.withholding_cap)),
            ("method".into(), variant(&self.method)),
            ("active".into(), Value::bool(self.active)),
        ])
    }
}

pub struct OssRegistration {
    pub home: String,
    pub vat_id_commitment: Bytes32,
    pub active: bool,
}

impl OssRegistration {
    pub fn to_value(&self) -> Value {
        Value::named_composite([
            ("home".into(), country(&self.home)),
            ("vat_id_commitment".into(), h32(&self.vat_id_commitment)),
            ("active".into(), Value::bool(self.active)),
        ])
    }
}

pub struct GrandpaAuthority {
    pub id: Bytes32,
    pub weight: u64,
}

pub struct GrandpaAuthoritySet {
    pub authorities: Vec<GrandpaAuthority>,
    pub set_id: u64,
}

impl GrandpaAuthoritySet {
    pub fn to_value(&self) -> Value {
        let auths = self.authorities.iter().map(|a| {
            Value::named_composite([
                ("id".into(), h32(&a.id)),
                ("weight".into(), u(a.weight as u128)),
            ])
        });
        Value::named_composite([
            ("authorities".into(), Value::unnamed_composite(auths)),
            ("set_id".into(), u(self.set_id as u128)),
        ])
    }
}

// Re-export the helpers most call sites need.
pub use self::{country as country_code};

/// Treasury allocation pool ids (§08).
pub mod pools {
    pub const STAKING: u8 = 0;
    pub const TREASURY: u8 = 1;
    pub const SUBSIDY: u8 = 2;
    pub const DEV: u8 = 3;
    pub const ECOSYSTEM: u8 = 4;
}
