# ferrum-sdk — Rust

Typed thin wrapper over [`subxt`](https://github.com/paritytech/subxt) for the
Ferrum sovereign blockchain. Uses subxt's **dynamic** API driven by on-chain
metadata — no codegen step and no committed `metadata.scale` file, so the SDK can
never drift from a stale metadata snapshot.

## Add to your project

```toml
[dependencies]
ferrum-sdk = { path = "../sdk/rust" }   # or a published version
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

## Quickstart

```rust
use ferrum_sdk::{dev, types::*, FerrumClient};
use subxt::utils::AccountId32;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let ferrum = FerrumClient::connect("ws://127.0.0.1:9944").await?;
    let alice = dev::alice();
    let alice_id: AccountId32 = alice.public_key().to_account_id();

    let doc = DidDocument {
        did: Did::new("tw", "citizen-0001"),
        controller: alice_id,
        doc_hash: [0u8; 32],            // a commitment computed off-chain — no PII
        keys: vec![DidKeyRef { kind: "Sr25519".into(), key_hash: [0u8; 32] }],
        revocation_commitment: [0u8; 32],
        anchored_at: 0,
    };
    ferrum.sign_and_send(&ferrum.identity().anchor_did(&doc), &alice).await?;
    Ok(())
}
```

Run the full example against `ferrum-node --dev`:

```bash
cargo run --example quickstart
```

## API shape

`FerrumClient` exposes one namespace per pallet; each method returns a `FerrumCall`
(dynamic payload) submitted with `sign_and_send`:

```
ferrum.identity()     anchor_did · rotate_keys · update_revocation · register_issuer
ferrum.credential()   issue · revoke · set_status · log_presentation
ferrum.tax()          anchor_invoice · withhold · file_obligation · prove_bracket · settle · authorize_audit · set_brackets
ferrum.treasury()     mint · burn · subsidize · record_settlement
ferrum.federation()   propose · vote · close · set_membership · set_basket · mint_xsu · redeem_xsu · book_clearing · net_and_settle · publish_proof_of_reserve
ferrum.interop()      register_issuer · submit_instruction · verify_finality · net_and_settle · register_validator · slash_validator
                      init_authority_set · rotate_authority_set · register_issuer_vk · verify_foreign_proof · register_treaty
                      recognize_foreign_invoice · oss_register · oss_report
```

### Storage and events

```rust
let q = subxt::dynamic::storage("Treasury", "TotalBurned", Vec::<scale_value::Value>::new());
let burned = ferrum.storage().at_latest().await?.fetch(&q).await?;

let events = ferrum.sign_and_send(&call, &alice).await?;
for ev in events.iter() { /* inspect ev?.pallet_name(), ev?.variant_name() */ }
```

### Conventions

- 32-byte fields are `[u8; 32]`; arbitrary blobs (proof/vk/finality) are `&[u8]`.
- Tags/country/currency are `&str` (`"tw"`, `"TW"`, `"TWD"`) and length-checked.
- Variant fields (`kind`, `status`, `vote`, `method`) are `&str` variant names
  validated against metadata at encode time.
- Rates are `f64` fractions of one (`0.05` = 5%); amounts are `u128`.
- `FederationAction` is a typed enum.

Personal-data fields only accept commitments/hashes — by design you cannot put
plaintext PII into an extrinsic (whitepaper §03/§05/§06/§09).

### Prefer static codegen?

For compile-time-checked types you can switch to the codegen path: export the
metadata once (`subxt metadata --url ws://127.0.0.1:9944 -f bytes > metadata.scale`)
and annotate a module with `#[subxt::subxt(runtime_metadata_path = "metadata.scale")]`.
The dynamic wrapper here is the zero-friction default.
