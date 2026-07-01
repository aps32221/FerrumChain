# Ferrum · Federation Governance Console / 聯邦治理主控台

A clean **ReactJS** console for **whitepaper Chapter 11 — Federation Governance &
Token Operations** (`pallet-federation`). Same architecture and design language as
`apps/interop-console` (Chapter 9).

針對白皮書**第 11 章「聯邦治理與代幣運作」**的 ReactJS 主控台,設計與架構沿用
`apps/interop-console`。

You operate the **treaty council as the TW seat**. Every panel maps **1:1 to a
real `pallet-federation` extrinsic** (module index `14`, call indices matching
`sdk/csharp/Ferrum.Sdk/Calls.cs` → `FederationCalls`), reproduces its validation
rules and `Error`s, and emits the same `Event`s — including the **dual-majority**
rule (`pallets/federation/src/voting.rs`) and the §11.2 governance domains.

## Run / 執行

```bash
cd apps/federation-console
npm install
npm run dev          # http://localhost:5191
# or: npm run build && npm run preview
```

Runs fully offline with a faithful in-browser model (seeded with 5 members, the
§10 illustrative XSU basket, a reserve pool and one open proposal). Connect to a
node + signer to submit real extrinsics via `author_submitExtrinsic`.

## What it covers / 涵蓋範圍

| View | § | Operations (extrinsics) |
|------|---|--------------------------|
| **Overview** | — | seats, basket weights, XSU issued, governance pipeline, neutrality invariant |
| **11.1 Council** | §11.1 | `setMembership` — seats, secretariat & validator notes |
| **11.2 Governance** | §11.2/§11.4 | `propose` (6 `FederationAction` variants) · `vote` · `close` — with **live dual-majority** (member axis + basket-weight axis) and the timelock queue → enact (`on_initialize`) |
| **XSU basket & reserve** | §10/§11.3 | `setBasket` (reweight editor, must sum to 100%) · `mintXsu` · `redeemXsu` |
| **Clearing & PoR** | §11.3 | `bookClearing` · `netAndSettle` · `publishProofOfReserve` |
| **Event log** | — | the emitted event stream |

The **dual-majority** test is computed live exactly as in `voting.rs`: a proposal
passes only if Aye voters clear **both** the member-count threshold and the summed
XSU basket-weight threshold for the action's governance domain (Parameter ⅔,
Membership/Dispute ¾, Constitutional ≥85%, …).

## Live submission / 真實上鏈

Identical to interop-console: `src/scale.js` (SCALE encoder), `src/encode.js`
(federation call + `FederationAction`/`XsuBasket` encoders, verified against fixed
test vectors), `src/rpc.js` (JSON-RPC + hand-assembled v4 signed extrinsic →
`author_submitExtrinsic`). In live mode a vote is cast as the **connected seat**
(the on-chain `vote` takes only `(id, vote)`; the member is the signing origin),
and timelock enactment happens on-chain via `on_initialize` (the sim's
“fast-forward to ETA” button is offline-only).

## Layout / 檔案

```
src/
  chain.js          in-browser pallet-federation model (calls, dual-majority, events)
  scale.js          SCALE encoder (shared with interop-console)
  encode.js         encodeCall(name,args) + FederationAction/XsuBasket encoders
  rpc.js            JSON-RPC + v4 extrinsic signer + author_submitExtrinsic
  store.jsx         React context: dispatch (live or sim), connection, event log
  format.js         XSU/FER units, member codes, Perbill helpers
  ui.jsx            reusable primitives (shared design with interop-console)
  views/            Overview · Council · Proposals · Treasury · Clearing · EventLog
  App.jsx           shell: top bar, connection bar, sidebar nav, toast
```

> Adding a member: add it to `MEMBERS` in `format.js` (it flows into every
> selector); seed seats/weights in `chain.js` if you want it pre-loaded.
