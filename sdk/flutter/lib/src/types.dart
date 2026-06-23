import 'dart:typed_data';

import 'scale_writer.dart';

// Enum index = SCALE discriminant (matches the Rust declaration order).
enum KeyKind { sr25519, ed25519, bls12_381 }

enum CredentialKind { nationality, age, residence, taxStatus, other }

enum CredentialStatus { active, suspended, revoked, expired }

enum TaxKind { income, wage, interest, valueAdded, withholding, other }

enum Vote { aye, nay, abstain }

enum XcmStatus { pending, finalityVerified, accepted, rejected }

enum CreditMethod { credit, exemption }

/// did:fer identifier. Tag/id given as ASCII; bytes are derived.
class Did {
  final String chainTag;
  final Uint8List id;
  Did(this.chainTag, this.id);
  factory Did.of(String chainTag, String id) =>
      Did(chainTag, Uint8List.fromList(id.codeUnits));
  void encode(ScaleWriter w) => w.asciiVec(chainTag).bytes(id);
}

class DidKeyRef {
  final KeyKind kind;
  final String keyHashHex;
  DidKeyRef(this.kind, this.keyHashHex);
  void encode(ScaleWriter w) => w.u8(kind.index).fixed(hex32(keyHashHex));
}

class DidDocument {
  final Did did;
  final Uint8List controller;
  final String docHashHex;
  final List<DidKeyRef> keys;
  final String revocationCommitmentHex;
  final int anchoredAt;
  DidDocument(this.did, this.controller, this.docHashHex, this.keys,
      this.revocationCommitmentHex, this.anchoredAt);
  void encode(ScaleWriter w) {
    did.encode(w);
    w.fixed(controller).fixed(hex32(docHashHex));
    w.vec(keys, (ww, k) => k.encode(ww));
    w.fixed(hex32(revocationCommitmentHex)).u32(anchoredAt);
  }
}

class FiatAmount {
  final String currency;
  final BigInt minorUnits;
  FiatAmount(this.currency, this.minorUnits);
  void encode(ScaleWriter w) => w.asciiFixed(currency, 3).u128(minorUnits);
}

class CredentialAnchor {
  final Did subject;
  final Uint8List issuer;
  final CredentialKind kind;
  final String payloadHashHex;
  final CredentialStatus status;
  final int? expiresAt;
  CredentialAnchor(this.subject, this.issuer, this.kind, this.payloadHashHex,
      this.status, this.expiresAt);
  void encode(ScaleWriter w) {
    subject.encode(w);
    w.fixed(issuer).u8(kind.index).fixed(hex32(payloadHashHex)).u8(status.index);
    w.option<int>(expiresAt, (ww, v) => ww.u64(v));
  }
}

class TaxBracket {
  final int index;
  final double rate;
  TaxBracket(this.index, this.rate);
  void encode(ScaleWriter w) => w.u8(index).u32(perbill(rate));
}

class InvoiceAnchor {
  final String invoiceHashHex;
  final Uint8List issuer;
  final TaxKind kind;
  final int anchoredAt;
  InvoiceAnchor(this.invoiceHashHex, this.issuer, this.kind, this.anchoredAt);
  void encode(ScaleWriter w) =>
      w.fixed(hex32(invoiceHashHex)).fixed(issuer).u8(kind.index).u64(anchoredAt);
}

class TaxObligation {
  final Did subject;
  final TaxKind kind;
  final FiatAmount amountDue;
  final String detailCommitmentHex;
  final bool settled;
  TaxObligation(this.subject, this.kind, this.amountDue, this.detailCommitmentHex,
      this.settled);
  void encode(ScaleWriter w) {
    subject.encode(w);
    w.u8(kind.index);
    amountDue.encode(w);
    w.fixed(hex32(detailCommitmentHex)).boolean(settled);
  }
}

class AgeProofPublicInputs {
  final String issuerCommitmentHex;
  final int threshold;
  final String nullifierHex;
  AgeProofPublicInputs(this.issuerCommitmentHex, this.threshold, this.nullifierHex);
  void encode(ScaleWriter w) =>
      w.fixed(hex32(issuerCommitmentHex)).u32(threshold).fixed(hex32(nullifierHex));
}

class BasketEntry {
  final String cbdc;
  final double weight;
  BasketEntry(this.cbdc, this.weight);
  void encode(ScaleWriter w) => w.asciiFixed(cbdc, 3).u32(perbill(weight));
}

class XsuBasket {
  final List<BasketEntry> entries;
  final int version;
  XsuBasket(this.entries, this.version);
  void encode(ScaleWriter w) => w.vec(entries, (ww, e) => e.encode(ww)).u32(version);
}

abstract class FederationAction {
  void encode(ScaleWriter w);
  factory FederationAction.setParameter(String key, BigInt value) = _SetParameter;
  factory FederationAction.admitMember(String member) = _AdmitMember;
  factory FederationAction.removeMember(String member) = _RemoveMember;
  factory FederationAction.reweight(XsuBasket basket) = _Reweight;
  factory FederationAction.suspendMember(String member) = _SuspendMember;
  factory FederationAction.runtimeUpgrade(String codeHashHex) = _RuntimeUpgrade;
}

class _SetParameter implements FederationAction {
  final String key;
  final BigInt value;
  _SetParameter(this.key, this.value);
  @override
  void encode(ScaleWriter w) => w.u8(0).asciiVec(key).u128(value);
}

class _AdmitMember implements FederationAction {
  final String member;
  _AdmitMember(this.member);
  @override
  void encode(ScaleWriter w) => w.u8(1).asciiFixed(member, 2);
}

class _RemoveMember implements FederationAction {
  final String member;
  _RemoveMember(this.member);
  @override
  void encode(ScaleWriter w) => w.u8(2).asciiFixed(member, 2);
}

class _Reweight implements FederationAction {
  final XsuBasket basket;
  _Reweight(this.basket);
  @override
  void encode(ScaleWriter w) {
    w.u8(3);
    basket.encode(w);
  }
}

class _SuspendMember implements FederationAction {
  final String member;
  _SuspendMember(this.member);
  @override
  void encode(ScaleWriter w) => w.u8(4).asciiFixed(member, 2);
}

class _RuntimeUpgrade implements FederationAction {
  final String codeHashHex;
  _RuntimeUpgrade(this.codeHashHex);
  @override
  void encode(ScaleWriter w) => w.u8(5).fixed(hex32(codeHashHex));
}

class TrustRegistryEntry {
  final String country;
  final String issuerKeyHashHex;
  final String scope;
  final bool active;
  TrustRegistryEntry(this.country, this.issuerKeyHashHex, this.scope, this.active);
  void encode(ScaleWriter w) => w
      .asciiFixed(country, 2)
      .fixed(hex32(issuerKeyHashHex))
      .asciiVec(scope)
      .boolean(active);
}

class ClearingInstruction {
  final String from;
  final String to;
  final BigInt amount;
  final String detailCommitmentHex;
  final XcmStatus status;
  ClearingInstruction(this.from, this.to, this.amount, this.detailCommitmentHex,
      {this.status = XcmStatus.pending});
  void encode(ScaleWriter w) => w
      .asciiFixed(from, 2)
      .asciiFixed(to, 2)
      .u128(amount)
      .fixed(hex32(detailCommitmentHex))
      .u8(status.index);
}

class TaxTreaty {
  final double withholdingCap;
  final CreditMethod method;
  final bool active;
  TaxTreaty(this.withholdingCap, this.method, this.active);
  void encode(ScaleWriter w) =>
      w.u32(perbill(withholdingCap)).u8(method.index).boolean(active);
}

class OssRegistration {
  final String home;
  final String vatIdCommitmentHex;
  final bool active;
  OssRegistration(this.home, this.vatIdCommitmentHex, this.active);
  void encode(ScaleWriter w) =>
      w.asciiFixed(home, 2).fixed(hex32(vatIdCommitmentHex)).boolean(active);
}

class GrandpaAuthority {
  final String idHex;
  final int weight;
  GrandpaAuthority(this.idHex, this.weight);
  void encode(ScaleWriter w) => w.fixed(hex32(idHex)).u64(weight);
}

class GrandpaAuthoritySet {
  final List<GrandpaAuthority> authorities;
  final int setId;
  GrandpaAuthoritySet(this.authorities, this.setId);
  void encode(ScaleWriter w) =>
      w.vec(authorities, (ww, a) => a.encode(ww)).u64(setId);
}

/// Treasury allocation pool ids (§08).
class Pools {
  static const int staking = 0, treasury = 1, subsidy = 2, dev = 3, ecosystem = 4;
}
