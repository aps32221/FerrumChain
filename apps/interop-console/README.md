# Ferrum · Cross-border Interop Console / 跨境互通操作主控台

A clean **ReactJS** operations console for **whitepaper Chapter 9 — cross-border
interop operations** (`pallet-interop`, runbook `BUILD.md §3.9`).

針對白皮書**第 9 章「跨境互通操作」**的 ReactJS 操作主控台,對應 `pallet-interop`
與 `BUILD.md §3.9` 操作手冊。

It puts the entire §3.9 runbook behind a simple operator UI: you act as the **TW**
sovereign chain, bridging and clearing against **JP / DE / US / CN**. Every panel
maps **1:1 to a real `pallet-interop` extrinsic** (module index `15`, call indices
matching `sdk/csharp/Ferrum.Sdk/Calls.cs`), reproduces that call's validation
rules and the exact `Error` it raises, and emits the same on-chain `Event`s.

## Run / 執行

```bash
cd apps/interop-console
npm install
npm run dev          # http://localhost:5190
# or: npm run build && npm run preview
```

The console runs in **two modes**:

- **Simulation (default).** A faithful in-browser model of `pallet-interop` —
  rehearse the full §3.9 runbook offline, no node required. Seeds two chains (TW
  operating, JP pre-bridged) so every flow works immediately.
- **Live.** Enter a node WebSocket endpoint + a signer (`//Alice` on a `--dev`
  chain, or a `0x` secret seed) in the connection bar and click **Connect node**.
  Each operation then **SCALE-encodes the call, signs a v4 extrinsic, and submits
  it via the raw `author_submitExtrinsic` JSON-RPC method** — surfacing the real
  extrinsic hash.

### Live submission internals / 真實上鏈

The live path (`src/encode.js` + `src/rpc.js`) is a byte-for-byte port of the C#
SDK's extrinsic assembly (`Ferrum.Sdk/FerrumClient.cs`, `Calls.cs`, `Scale.cs`):

- **Call** = `module (15) ++ callIndex ++ params`, using the call indices in
  `CALLS` and the same SCALE encoders (verified against fixed test vectors).
- **Signed extrinsic v4** = `0x84 ++ MultiAddress::Id(pubkey) ++
  MultiSignature::Sr25519(sig) ++ extra ++ call`, length-prefixed.
  - `extra` = immortal era `0x00` ++ `compact(nonce)` ++ `compact(tip=0)`
  - additional signed = `specVersion ++ txVersion ++ genesis ++ genesis`
  - payload is blake2-256-hashed before signing when > 256 bytes
  - **no `CheckMetadataHash`** extension — matching this runtime's 8-field
    `SignedExtra` exactly (the reason the SDK hand-assembles rather than using a
    generic builder).
- Chain constants come from `chain_getBlockHash(0)`, `state_getRuntimeVersion`
  and `system_accountNextIndex`; signing uses sr25519 via `@polkadot/keyring`.

Proof-bearing calls (`verifyFinality`, `rotateAuthoritySet`, `verifyForeignProof`)
reveal extra fields in live mode for the operator to paste the SCALE/arkworks
proof blobs the runtime actually verifies. After a successful submit the local
view is mirrored optimistically; live on-chain state is authoritative.

## What it covers / 涵蓋範圍

The navigation mirrors the three §3.9 capability groups plus validators:

| View | §3.9 | Operations (extrinsics) |
|------|------|--------------------------|
| **A · Bridge** | A | `submitInstruction` · `verifyFinality` · `rotateAuthoritySet` · `netAndSettle` — trust-minimized GRANDPA light client, XSU multilateral netting |
| **B · Identity** | B | `registerIssuer` · `registerIssuerVk` · `verifyForeignProof` + read-only `resolveDid` — trust registry, cross-chain DID resolution, cross-border ZK verify |
| **C · Tax** | C | `registerTreaty` · `recognizeForeignInvoice` · `ossRegister` · `ossReport` — double-tax relief, e-invoice recognition, OSS VAT |
| **Validators** | §11.1 | `registerValidator` · `slashValidator` — national-FER cross-slashable bonds |
| **Overview / Event log** | — | federation status, net positions, and the emitted event stream |

Faithful error paths include `StaleFinality`, `NonSequentialSetId`,
`IssuerNotRecognized`, `VerifyingKeyNotFound`, `ProofReplayed`, `NoFinalizedHead`,
`InsufficientBond`, `SlashExceedsBond` — surfaced as the operator submits.

## Privacy invariant / 隱私不變式 (§09)

The UI only ever handles **commitments, hashes, finality/ZK proofs and XSU net
amounts** — never plaintext PII, exactly as the on-chain pallet enforces.

## Layout / 檔案

```
src/
  chain.js          in-browser pallet-interop model (calls, validation, events)
  scale.js          SCALE encoder (port of the C# SDK ScaleWriter)
  encode.js         encodeCall(name,args) → module/call bytes + UI→tx adapter
  rpc.js            JSON-RPC client + v4 extrinsic signer + author_submitExtrinsic
  store.jsx         React context: dispatch (live or sim), connection, event log
  format.js         FER/XSU units, country codes, hash helpers
  ui.jsx            reusable primitives (Card, OpForm, Table, Pill, …)
  views/            Overview · Bridge · Identity · Tax · Validators · EventLog
  App.jsx           shell: top bar, connection bar, sidebar nav, toast
```

> Live mode signs with a seed/URI (dev accounts or a raw secret seed); browser
> wallet-extension signing is not wired yet. Live tables update optimistically —
> read-back via storage queries (`state_getStorage`) is the natural next step.
