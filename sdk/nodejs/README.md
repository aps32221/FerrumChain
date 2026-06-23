# @ferrum/sdk — Node.js / TypeScript

Typed thin wrapper over [`@polkadot/api`](https://github.com/polkadot-js/api) for the
Ferrum sovereign blockchain.

## Install

```bash
npm install @ferrum/sdk @polkadot/api
```

## Quickstart

```ts
import { FerrumClient } from "@ferrum/sdk";
import { blake2AsHex } from "@polkadot/util-crypto";

const ferrum = await FerrumClient.connect({ endpoint: "ws://127.0.0.1:9944" });
const alice = ferrum.keypair("//Alice");

const tx = ferrum.identity.anchorDid({
  did: { chainTag: "tw", id: "citizen-0001" },
  controller: alice.address,
  docHash: blake2AsHex("off-chain DID document", 256), // commitment only — no PII
  keys: [{ kind: "Sr25519", keyHash: blake2AsHex("device-key", 256) }],
  revocationCommitment: blake2AsHex("rev-acc-0", 256),
  anchoredAt: 0,
});
await ferrum.signAndSend(tx, alice);
await ferrum.disconnect();
```

Run the full example against `ferrum-node --dev`:

```bash
npm install
npm run example
```

## API shape

`FerrumClient` exposes one namespace per pallet; every method returns a
`SubmittableExtrinsic` you can sign and send yourself or via `signAndSend`:

```
ferrum.identity     anchorDid · rotateKeys · updateRevocation · registerIssuer
ferrum.credential   issue · revoke · setStatus · logPresentation
ferrum.tax          anchorInvoice · withhold · fileObligation · proveBracket · settle · authorizeAudit · setBrackets
ferrum.treasury     mint · burn · subsidize · recordSettlement
ferrum.federation   propose · vote · close · setMembership · setBasket · mintXsu · redeemXsu · bookClearing · netAndSettle · publishProofOfReserve
ferrum.interop      registerIssuer · submitInstruction · verifyFinality · netAndSettle · registerValidator · slashValidator
                    initAuthoritySet · rotateAuthoritySet · registerIssuerVk · verifyForeignProof · registerTreaty
                    recognizeForeignInvoice · ossRegister · ossReport
```

### Storage and events

```ts
const doc = await ferrum.query.identity.dids({ chainTag: "tw", id: "citizen-0001" });
const burned = await ferrum.query.treasury.totalBurned();

const unsub = await ferrum.subscribeEvents((e) => {
  if (e.section === "tax") console.log(e.method, e.data);
});
```

### Input conventions

- 32-byte fields (`docHash`, `payloadHash`, `detailCommitment`, nullifiers, key
  hashes) accept a `0x…` hex string or a 32-byte `Uint8Array`.
- Tags/country/currency (`chainTag`, `country`, `cbdc`, `currency`) accept short
  ASCII strings (`"tw"`, `"TW"`, `"TWD"`).
- Rates (`Perbill`) accept a fraction of one: `0.05` = 5%.
- Amounts accept `bigint`, `number` or a decimal string (FER has 12 decimals).

The SDK only accepts commitments/hashes for personal-data fields — by design you
cannot place plaintext PII into an extrinsic (whitepaper §03/§05/§06/§09).
