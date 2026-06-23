//! Quickstart against a local dev node:  ./target/release/ferrum-node --dev
//!
//!     cargo run --example quickstart
//!
//! Accredits Alice as an issuer (via sudo), anchors a DID, files a tax obligation.

use blake2::{Blake2b, Digest};
use blake2::digest::consts::U32;
use ferrum_sdk::{dev, types::*, FerrumClient};
use subxt::utils::AccountId32;

fn commit(s: &str) -> [u8; 32] {
    let mut h = Blake2b::<U32>::new();
    h.update(s.as_bytes());
    h.finalize().into()
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let ferrum = FerrumClient::connect("ws://127.0.0.1:9944").await?;
    let alice = dev::alice();
    let alice_id: AccountId32 = alice.public_key().to_account_id();

    let subject = Did::new("tw", "citizen-0001");

    // 1. Governance accredits Alice as an issuer (sudo wraps governance on --dev).
    //    Nest the inner call as a RuntimeCall value: outer variant = pallet name,
    //    inner variant = call name + named fields.
    use subxt::ext::scale_value::Value;
    let pallet_call = Value::named_variant("register_issuer", [("who".into(), account(&alice_id))]);
    let runtime_call = Value::unnamed_variant("Identity", [pallet_call]);
    let sudo = subxt::dynamic::tx("Sudo", "sudo", vec![runtime_call]);
    ferrum.sign_and_send(&sudo, &alice).await?;
    println!("issuer accredited");

    // 2. Anchor the DID — doc_hash is a commitment, never the document itself.
    let doc = DidDocument {
        did: subject.clone(),
        controller: alice_id.clone(),
        doc_hash: commit("off-chain DID document for citizen #1"),
        keys: vec![DidKeyRef { kind: "Sr25519".into(), key_hash: commit("device-key") }],
        revocation_commitment: commit("rev-acc-0"),
        anchored_at: 0,
    };
    ferrum.sign_and_send(&ferrum.identity().anchor_did(&doc), &alice).await?;
    println!("DID anchored");

    // 3. File a fiat-denominated tax obligation.
    let obligation = TaxObligation {
        subject: subject.clone(),
        kind: "Income".into(),
        amount_due: FiatAmount { currency: "TWD".into(), minor_units: 1_234_500 },
        detail_commitment: commit("encrypted return detail"),
        settled: false,
    };
    ferrum.sign_and_send(&ferrum.tax().file_obligation(&obligation), &alice).await?;
    println!("obligation filed");

    Ok(())
}
