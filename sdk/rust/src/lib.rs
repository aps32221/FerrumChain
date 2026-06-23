//! # ferrum-sdk — typed thin wrapper over [`subxt`]
//!
//! Connect to a Ferrum node and build/sign/submit any extrinsic across the six
//! pallets via the dynamic subxt API (driven by on-chain metadata — no codegen
//! step). Each namespace method returns a [`FerrumCall`] you submit with
//! [`FerrumClient::sign_and_send`].

pub mod types;

use subxt::ext::scale_value::Value;
use subxt::tx::DefaultPayload;
use subxt::utils::AccountId32;
use subxt::{OnlineClient, SubstrateConfig};
use subxt_signer::sr25519::Keypair;

pub use subxt_signer::sr25519::dev;
pub use types::*;

pub const DEFAULT_ENDPOINT: &str = "ws://127.0.0.1:9944";

/// The concrete dynamic-extrinsic payload type returned by every namespace method.
pub type FerrumCall = DefaultPayload<subxt::ext::scale_value::Composite<()>>;

fn call(pallet: &str, name: &str, fields: Vec<Value>) -> FerrumCall {
    subxt::dynamic::tx(pallet, name, fields)
}

/// A connected Ferrum client. Cheap to clone.
#[derive(Clone)]
pub struct FerrumClient {
    pub api: OnlineClient<SubstrateConfig>,
}

impl FerrumClient {
    /// Dial a node over WebSocket.
    pub async fn connect(endpoint: &str) -> Result<Self, subxt::Error> {
        Ok(Self { api: OnlineClient::<SubstrateConfig>::from_url(endpoint).await? })
    }

    pub fn identity(&self) -> Identity { Identity }
    pub fn credential(&self) -> Credential { Credential }
    pub fn tax(&self) -> Tax { Tax }
    pub fn treasury(&self) -> Treasury { Treasury }
    pub fn federation(&self) -> Federation { Federation }
    pub fn interop(&self) -> Interop { Interop }

    /// Sign, submit, and wait for a finalized, successful inclusion.
    pub async fn sign_and_send(
        &self,
        call: &FerrumCall,
        signer: &Keypair,
    ) -> Result<subxt::blocks::ExtrinsicEvents<SubstrateConfig>, subxt::Error> {
        self.api
            .tx()
            .sign_and_submit_then_watch_default(call, signer)
            .await?
            .wait_for_finalized_success()
            .await
    }

    /// Access dynamic storage queries: `client.storage().at_latest().await?.fetch(&query).await?`.
    pub fn storage(&self) -> subxt::storage::StorageClient<SubstrateConfig, OnlineClient<SubstrateConfig>> {
        self.api.storage()
    }
}

// ---------------------------------------------------------------------------
// Pallet namespaces.
// ---------------------------------------------------------------------------

pub struct Identity;
impl Identity {
    pub fn anchor_did(&self, doc: &DidDocument) -> FerrumCall { call("Identity", "anchor_did", vec![doc.to_value()]) }
    pub fn rotate_keys(&self, did: &Did, keys: &[DidKeyRef]) -> FerrumCall {
        let keys = Value::unnamed_composite(keys.iter().map(DidKeyRef::to_value));
        call("Identity", "rotate_keys", vec![did.to_value(), keys])
    }
    pub fn update_revocation(&self, commitment: &Bytes32) -> FerrumCall { call("Identity", "update_revocation", vec![h32(commitment)]) }
    pub fn register_issuer(&self, who: &AccountId32) -> FerrumCall { call("Identity", "register_issuer", vec![account(who)]) }
}

pub struct Credential;
impl Credential {
    pub fn issue(&self, anchor: &CredentialAnchor) -> FerrumCall { call("Credential", "issue", vec![anchor.to_value()]) }
    pub fn revoke(&self, payload_hash: &Bytes32) -> FerrumCall { call("Credential", "revoke", vec![h32(payload_hash)]) }
    pub fn set_status(&self, payload_hash: &Bytes32, status: &str) -> FerrumCall {
        call("Credential", "set_status", vec![h32(payload_hash), Value::unnamed_variant(status, std::iter::empty::<Value>())])
    }
    pub fn log_presentation(&self, nullifier: &Bytes32, commitment: &Bytes32) -> FerrumCall {
        call("Credential", "log_presentation", vec![h32(nullifier), h32(commitment)])
    }
}

pub struct Tax;
impl Tax {
    pub fn anchor_invoice(&self, anchor: &InvoiceAnchor) -> FerrumCall { call("Tax", "anchor_invoice", vec![anchor.to_value()]) }
    pub fn withhold(&self, subject: &Did, kind: &str, amount: &FiatAmount) -> FerrumCall {
        call("Tax", "withhold", vec![subject.to_value(), Value::unnamed_variant(kind, std::iter::empty::<Value>()), amount.to_value()])
    }
    pub fn file_obligation(&self, obligation: &TaxObligation) -> FerrumCall { call("Tax", "file_obligation", vec![obligation.to_value()]) }
    pub fn prove_bracket(&self, proof: &[u8], inputs: &AgeProofPublicInputs) -> FerrumCall {
        call("Tax", "prove_bracket", vec![blob(proof), inputs.to_value()])
    }
    pub fn settle(&self, subject: &Did, slot: u64) -> FerrumCall {
        call("Tax", "settle", vec![subject.to_value(), Value::u128(slot as u128)])
    }
    pub fn authorize_audit(&self, invoice: &Bytes32, viewing_key_commitment: &Bytes32) -> FerrumCall {
        call("Tax", "authorize_audit", vec![h32(invoice), h32(viewing_key_commitment)])
    }
    pub fn set_brackets(&self, brackets: &[TaxBracket]) -> FerrumCall {
        call("Tax", "set_brackets", vec![Value::unnamed_composite(brackets.iter().map(TaxBracket::to_value))])
    }
}

pub struct Treasury;
impl Treasury {
    pub fn mint(&self, pool: u8, amount: u128) -> FerrumCall { call("Treasury", "mint", vec![Value::u128(pool as u128), Value::u128(amount)]) }
    pub fn burn(&self, amount: u128) -> FerrumCall { call("Treasury", "burn", vec![Value::u128(amount)]) }
    pub fn subsidize(&self, who: &AccountId32, amount: u128) -> FerrumCall { call("Treasury", "subsidize", vec![account(who), Value::u128(amount)]) }
    pub fn record_settlement(&self, receipt: &Bytes32, amount: &FiatAmount) -> FerrumCall {
        call("Treasury", "record_settlement", vec![h32(receipt), amount.to_value()])
    }
}

pub struct Federation;
impl Federation {
    pub fn propose(&self, action: &FederationAction) -> FerrumCall { call("Federation", "propose", vec![action.to_value()]) }
    pub fn vote(&self, id: u64, vote: &str) -> FerrumCall {
        call("Federation", "vote", vec![Value::u128(id as u128), Value::unnamed_variant(vote, std::iter::empty::<Value>())])
    }
    pub fn close(&self, id: u64) -> FerrumCall { call("Federation", "close", vec![Value::u128(id as u128)]) }
    pub fn set_membership(&self, member: &str, seated: bool) -> FerrumCall {
        call("Federation", "set_membership", vec![country_code(member), Value::bool(seated)])
    }
    pub fn set_basket(&self, basket: &XsuBasket) -> FerrumCall { call("Federation", "set_basket", vec![basket.to_value()]) }
    pub fn mint_xsu(&self, cbdc: &str, cbdc_amount: u128) -> FerrumCall {
        assert_eq!(cbdc.len(), 3, "cbdc must be 3 ASCII bytes");
        call("Federation", "mint_xsu", vec![blob(cbdc.as_bytes()), Value::u128(cbdc_amount)])
    }
    pub fn redeem_xsu(&self, cbdc: &str, xsu_amount: u128) -> FerrumCall {
        assert_eq!(cbdc.len(), 3, "cbdc must be 3 ASCII bytes");
        call("Federation", "redeem_xsu", vec![blob(cbdc.as_bytes()), Value::u128(xsu_amount)])
    }
    pub fn book_clearing(&self, to: &str, amount: u128) -> FerrumCall {
        call("Federation", "book_clearing", vec![country_code(to), Value::u128(amount)])
    }
    pub fn net_and_settle(&self, window: u32) -> FerrumCall { call("Federation", "net_and_settle", vec![Value::u128(window as u128)]) }
    pub fn publish_proof_of_reserve(&self) -> FerrumCall { call("Federation", "publish_proof_of_reserve", vec![]) }
}

pub struct Interop;
impl Interop {
    pub fn register_issuer(&self, entry: &TrustRegistryEntry) -> FerrumCall { call("Interop", "register_issuer", vec![entry.to_value()]) }
    pub fn submit_instruction(&self, instr: &ClearingInstruction) -> FerrumCall { call("Interop", "submit_instruction", vec![instr.to_value()]) }
    pub fn verify_finality(&self, id: u64, finality_proof: &[u8]) -> FerrumCall {
        call("Interop", "verify_finality", vec![Value::u128(id as u128), blob(finality_proof)])
    }
    pub fn net_and_settle(&self, window: u32) -> FerrumCall { call("Interop", "net_and_settle", vec![Value::u128(window as u128)]) }
    pub fn register_validator(&self, bond: u128) -> FerrumCall { call("Interop", "register_validator", vec![Value::u128(bond)]) }
    pub fn slash_validator(&self, who: &AccountId32, amount: u128) -> FerrumCall {
        call("Interop", "slash_validator", vec![account(who), Value::u128(amount)])
    }
    pub fn init_authority_set(&self, country: &str, set: &GrandpaAuthoritySet) -> FerrumCall {
        call("Interop", "init_authority_set", vec![country_code(country), set.to_value()])
    }
    pub fn rotate_authority_set(&self, country: &str, finality_proof: &[u8]) -> FerrumCall {
        call("Interop", "rotate_authority_set", vec![country_code(country), blob(finality_proof)])
    }
    pub fn register_issuer_vk(&self, country: &str, issuer_key_hash: &Bytes32, vk: &[u8]) -> FerrumCall {
        call("Interop", "register_issuer_vk", vec![country_code(country), h32(issuer_key_hash), blob(vk)])
    }
    pub fn verify_foreign_proof(&self, country: &str, issuer_key_hash: &Bytes32, proof: &[u8], inputs: &AgeProofPublicInputs) -> FerrumCall {
        call("Interop", "verify_foreign_proof", vec![country_code(country), h32(issuer_key_hash), blob(proof), inputs.to_value()])
    }
    pub fn register_treaty(&self, a: &str, b: &str, treaty: &TaxTreaty) -> FerrumCall {
        call("Interop", "register_treaty", vec![country_code(a), country_code(b), treaty.to_value()])
    }
    pub fn recognize_foreign_invoice(&self, country: &str, invoice_hash: &Bytes32) -> FerrumCall {
        call("Interop", "recognize_foreign_invoice", vec![country_code(country), h32(invoice_hash)])
    }
    pub fn oss_register(&self, subject: &Did, registration: &OssRegistration) -> FerrumCall {
        call("Interop", "oss_register", vec![subject.to_value(), registration.to_value()])
    }
    pub fn oss_report(&self, subject: &Did, to: &str, amount: u128, detail_commitment: &Bytes32) -> FerrumCall {
        call("Interop", "oss_report", vec![subject.to_value(), country_code(to), Value::u128(amount), h32(detail_commitment)])
    }
}
