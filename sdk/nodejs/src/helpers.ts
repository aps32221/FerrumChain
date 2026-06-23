/**
 * Converters from the friendly input shapes (`types.ts`) to the registry-ready
 * objects `@polkadot/api` accepts for the Ferrum SCALE types.
 */
import { stringToU8a, u8aToHex, hexToU8a, isHex, isU8a, BN } from "@polkadot/util";
import type {
  DidInput, DidKeyRefInput, DidDocumentInput, CredentialAnchorInput, FiatAmountInput,
  TaxBracketInput, InvoiceAnchorInput, TaxObligationInput, AgeProofPublicInputs,
  XsuBasketInput, FederationActionInput, TrustRegistryEntryInput, ClearingInstructionInput,
  TaxTreatyInput, OssRegistrationInput, GrandpaAuthoritySetInput, Bytes32, ByteInput, Amount,
} from "./types.js";

/** Normalize any 32-byte input to `0x…` hex; throws on wrong length. */
export function h32(v: Bytes32): string {
  const u8 = isU8a(v) ? v : isHex(v) ? hexToU8a(v) : hexToU8a(("0x" + v) as `0x${string}`);
  if (u8.length !== 32) throw new Error(`expected 32 bytes, got ${u8.length}`);
  return u8aToHex(u8);
}

/** Normalize an arbitrary byte blob (proof/vk/finality) to `0x…` hex. */
export function bytes(v: ByteInput): string {
  return u8aToHex(isU8a(v) ? v : hexToU8a(v as `0x${string}`));
}

/** Encode a short ASCII string (tag/country/currency) to `0x…` byte hex. */
export function ascii(s: string): string {
  return u8aToHex(stringToU8a(s));
}

/** Encode a fixed-width ASCII code (e.g. "TWD"->3, "TW"->2) as a byte array. */
export function fixedAscii(s: string, len: number): number[] {
  const u8 = stringToU8a(s);
  if (u8.length !== len) throw new Error(`code "${s}" must be exactly ${len} ASCII bytes`);
  return Array.from(u8);
}

/** Fraction of one (0..1) -> Perbill parts-per-billion integer. */
export function perbill(frac: number): number {
  if (frac < 0 || frac > 1) throw new Error("perbill fraction must be within [0,1]");
  return Math.round(frac * 1_000_000_000);
}

/** Normalize an amount to a string the codec accepts losslessly. */
export function amount(a: Amount): string {
  return new BN(a.toString()).toString();
}

export function did(d: DidInput) {
  const id = typeof d.id === "string" && !isHex(d.id) ? ascii(d.id) : bytes(d.id as ByteInput);
  return { chainTag: ascii(d.chainTag), id };
}

export function didKeyRef(k: DidKeyRefInput) {
  return { kind: k.kind, keyHash: h32(k.keyHash) };
}

export function didDocument(d: DidDocumentInput) {
  return {
    did: did(d.did),
    controller: d.controller,
    docHash: h32(d.docHash),
    keys: d.keys.map(didKeyRef),
    revocationCommitment: h32(d.revocationCommitment),
    anchoredAt: d.anchoredAt,
  };
}

export function fiatAmount(f: FiatAmountInput) {
  return { currency: fixedAscii(f.currency, 3), minorUnits: amount(f.minorUnits) };
}

export function credentialAnchor(c: CredentialAnchorInput) {
  return {
    subject: did(c.subject),
    issuer: c.issuer,
    kind: c.kind,
    payloadHash: h32(c.payloadHash),
    status: c.status,
    expiresAt: c.expiresAt == null ? null : amount(c.expiresAt),
  };
}

export function taxBracket(b: TaxBracketInput) {
  return { index: b.index, rate: perbill(b.rate) };
}

export function invoiceAnchor(i: InvoiceAnchorInput) {
  return { invoiceHash: h32(i.invoiceHash), issuer: i.issuer, kind: i.kind, anchoredAt: amount(i.anchoredAt) };
}

export function taxObligation(o: TaxObligationInput) {
  return {
    subject: did(o.subject),
    kind: o.kind,
    amountDue: fiatAmount(o.amountDue),
    detailCommitment: h32(o.detailCommitment),
    settled: o.settled,
  };
}

export function ageProofPublicInputs(p: AgeProofPublicInputs) {
  return { issuerCommitment: h32(p.issuerCommitment), threshold: p.threshold, nullifier: h32(p.nullifier) };
}

export function xsuBasket(b: XsuBasketInput) {
  return {
    entries: b.entries.map((e) => ({ cbdc: fixedAscii(e.cbdc, 3), weight: perbill(e.weight) })),
    version: b.version,
  };
}

export function federationAction(a: FederationActionInput) {
  switch (a.type) {
    case "SetParameter": return { SetParameter: { key: ascii(a.key), value: amount(a.value) } };
    case "AdmitMember": return { AdmitMember: { member: fixedAscii(a.member, 2) } };
    case "RemoveMember": return { RemoveMember: { member: fixedAscii(a.member, 2) } };
    case "SuspendMember": return { SuspendMember: { member: fixedAscii(a.member, 2) } };
    case "Reweight": return { Reweight: { basket: xsuBasket(a.basket) } };
    case "RuntimeUpgrade": return { RuntimeUpgrade: { codeHash: h32(a.codeHash) } };
  }
}

export function trustRegistryEntry(e: TrustRegistryEntryInput) {
  return {
    country: fixedAscii(e.country, 2),
    issuerKeyHash: h32(e.issuerKeyHash),
    scope: ascii(e.scope),
    active: e.active,
  };
}

export function clearingInstruction(c: ClearingInstructionInput) {
  return {
    from: fixedAscii(c.from, 2),
    to: fixedAscii(c.to, 2),
    amount: amount(c.amount),
    detailCommitment: h32(c.detailCommitment),
    status: c.status ?? "Pending",
  };
}

export function taxTreaty(t: TaxTreatyInput) {
  return { withholdingCap: perbill(t.withholdingCap), method: t.method, active: t.active };
}

export function ossRegistration(r: OssRegistrationInput) {
  return { home: fixedAscii(r.home, 2), vatIdCommitment: h32(r.vatIdCommitment), active: r.active };
}

export function grandpaAuthoritySet(s: GrandpaAuthoritySetInput) {
  return {
    authorities: s.authorities.map((a) => ({ id: h32(a.id), weight: amount(a.weight) })),
    setId: amount(s.setId),
  };
}
