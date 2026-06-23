/**
 * Quickstart against a local dev node:  ./target/release/ferrum-node --dev
 *
 *   npm install && npm run example
 *
 * Anchors a DID, accredits Alice as an issuer, then files a tax obligation.
 */
import { FerrumClient, Pools } from "../src/index.js";
import { blake2AsHex } from "@polkadot/util-crypto";

async function main() {
  const ferrum = await FerrumClient.connect({ endpoint: "ws://127.0.0.1:9944" });
  const alice = ferrum.keypair("//Alice"); // dev issuer/governance/sudo

  // doc_hash is a commitment computed off-chain — never the document itself.
  const docHash = blake2AsHex("off-chain DID document for citizen #1", 256);
  const subject = { chainTag: "tw", id: "citizen-0001" };

  // 1. Governance accredits Alice as an issuer (sudo wraps governance on --dev).
  const accredit = ferrum.api.tx.sudo.sudo(
    ferrum.identity.registerIssuer(alice.address),
  );
  await ferrum.signAndSend(accredit as any, alice);
  console.log("issuer accredited");

  // 2. Anchor the DID (issuer origin).
  const anchor = ferrum.identity.anchorDid({
    did: subject,
    controller: alice.address,
    docHash,
    keys: [{ kind: "Sr25519", keyHash: blake2AsHex("device-key", 256) }],
    revocationCommitment: blake2AsHex("rev-acc-0", 256),
    anchoredAt: 0,
  });
  await ferrum.signAndSend(anchor, alice);
  console.log("DID anchored");

  // 3. File a fiat-denominated tax obligation.
  const obligation = ferrum.tax.fileObligation({
    subject,
    kind: "Income",
    amountDue: { currency: "TWD", minorUnits: 1234500n },
    detailCommitment: blake2AsHex("encrypted return detail", 256),
    settled: false,
  });
  await ferrum.signAndSend(obligation, alice);
  console.log("obligation filed");

  // 4. Read it back from storage.
  const stored = await ferrum.query.tax.obligations([subject, 0]);
  console.log("obligation on-chain:", stored.toHuman());

  console.log("subsidy pool id:", Pools.subsidy);
  await ferrum.disconnect();
}

main().catch((e) => { console.error(e); process.exit(1); });
