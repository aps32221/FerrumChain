# Ferrum SDKs — 多語言第三方 SDK / Multi-language third-party SDKs

Typed, thin-wrapper client libraries for the **Ferrum 鐵鏈** sovereign blockchain in
six languages. Each SDK wraps the established Substrate client library for that
language and adds Ferrum-specific, typed methods for every extrinsic, storage item
and event across the six pallets (`identity`, `credential`, `tax`, `treasury`,
`federation`, `interop`).

| Language | Directory | Wraps | Package |
|----------|-----------|-------|---------|
| Node.js / TypeScript | [`nodejs/`](./nodejs) | [`@polkadot/api`](https://github.com/polkadot-js/api) | `@ferrum/sdk` |
| Python | [`python/`](./python) | [`substrate-interface`](https://github.com/polkascan/py-substrate-interface) | `ferrum-sdk` |
| Rust | [`rust/`](./rust) | [`subxt`](https://github.com/paritytech/subxt) | `ferrum-sdk` |
| Java | [`java/`](./java) | [`polkaj`](https://github.com/emeraldpay/polkaj) | `network.ferrum:ferrum-sdk` |
| C# / .NET | [`csharp/`](./csharp) | [`Substrate.NET.API`](https://github.com/SubstrateGaming/Substrate.NET.API) | `Ferrum.Sdk` |
| Flutter / Dart | [`flutter/`](./flutter) | [`polkadart`](https://github.com/leonardocustodio/polkadart) | `ferrum_sdk` |

## Why thin wrappers?

The Ferrum node exposes a standard Substrate JSON-RPC / WebSocket interface with
**SCALE-encoded** extrinsics and **on-chain metadata (V15)**. The heavy lifting —
SCALE codec, sr25519/ed25519 signing, RPC transport, extrinsic construction and
type decoding — is already solved correctly by the libraries above, which read the
chain metadata at connection time. Each Ferrum SDK therefore:

1. Connects to a node (`ws://127.0.0.1:9944` by default).
2. Exposes a `FerrumClient` with one namespace per pallet, and a typed method per
   extrinsic that forwards to the underlying `tx`/`compose_call` API.
3. Ships **constructor helpers** for the Ferrum value types (`Did`, `FiatAmount`,
   `XsuBasket`, `FederationAction`, …) so callers build correct SCALE inputs
   without memorizing field shapes.
4. Provides **storage query** helpers and **event subscription** helpers.

The authoritative surface is [`catalog.json`](./catalog.json) — every SDK is kept
in lockstep with it. The Rust source of truth lives in the workspace pallets
(`pallets/*/src/lib.rs`) and `crates/primitives/src/lib.rs`.

## The Ferrum surface at a glance

```
identity   anchorDid · rotateKeys · updateRevocation · registerIssuer
credential issue · revoke · setStatus · logPresentation
tax        anchorInvoice · withhold · fileObligation · proveBracket · settle · authorizeAudit · setBrackets
treasury   mint · burn · subsidize · recordSettlement
federation propose · vote · close · setMembership · setBasket · mintXsu · redeemXsu · bookClearing · netAndSettle · publishProofOfReserve
interop    registerIssuer · submitInstruction · verifyFinality · netAndSettle · registerValidator · slashValidator
           initAuthoritySet · rotateAuthoritySet · registerIssuerVk · verifyForeignProof · registerTreaty
           recognizeForeignInvoice · ossRegister · ossReport
```

## Privacy invariant (carried into every SDK)

Per whitepaper §03/§05/§06/§09, **no plaintext PII ever goes on-chain**. Every
SDK's type helpers accept only commitments / hashes (`[u8;32]`) and bounded tag
bytes for the personal-data fields (`doc_hash`, `payload_hash`, `detail_commitment`,
`vat_id_commitment`, …). Compute those hashes off-chain (BLAKE2b-256) from data
held in agency-run encrypted vaults; the SDKs deliberately give you no way to put a
cleartext name, birthdate or invoice line-item into an extrinsic.

## Common conventions across all SDKs

- **Bytes fields** (`Hash32`, `Commitment`, `Nullifier`, key hashes) are accepted
  as a `0x`-prefixed hex string or a raw 32-byte buffer.
- **Tag/country/currency fields** (`chain_tag`, `CountryId`, `FiatCurrency`,
  `CbdcCode`) are accepted as short ASCII strings (e.g. `"tw"`, `"TW"`, `"TWD"`) and
  encoded to their fixed/bounded byte form by the helper.
- **AccountId** is accepted as an SS58 address string.
- **Amounts** (`Balance`, `minor_units`, `XsuAmount`) are accepted as the language's
  big-integer type to avoid precision loss (FER has 12 decimals).
- Signing uses a keypair/signer created from a mnemonic, seed or keystore — the dev
  chain ships the standard `//Alice … //Ferdie` accounts.

Start with the per-language READMEs; each has a copy-paste "anchor a DID + file a
tax obligation" quickstart against `ferrum-node --dev`.
