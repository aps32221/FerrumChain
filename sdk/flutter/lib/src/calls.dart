import 'dart:typed_data';

import 'scale_writer.dart';
import 'types.dart';

const int _identity = 10,
    _credential = 11,
    _tax = 12,
    _treasury = 13,
    _federation = 14,
    _interop = 15;

/// An encoded call: pallet index, call index and SCALE param bytes.
class FerrumCall {
  final int moduleIndex;
  final int callIndex;
  final Uint8List params;
  FerrumCall(this.moduleIndex, this.callIndex, this.params);

  /// Full call bytes (module ++ call ++ args).
  Uint8List encode() {
    final out = Uint8List(2 + params.length);
    out[0] = moduleIndex;
    out[1] = callIndex;
    out.setRange(2, out.length, params);
    return out;
  }
}

FerrumCall _call(int module, int index, void Function(ScaleWriter) args) {
  final w = ScaleWriter();
  args(w);
  return FerrumCall(module, index, w.toBytes());
}

class IdentityCalls {
  FerrumCall anchorDid(DidDocument doc) => _call(_identity, 0, doc.encode);
  FerrumCall rotateKeys(Did did, List<DidKeyRef> keys) => _call(_identity, 1, (w) {
        did.encode(w);
        w.vec(keys, (ww, k) => k.encode(ww));
      });
  FerrumCall updateRevocation(String commitmentHex) =>
      _call(_identity, 2, (w) => w.fixed(hex32(commitmentHex)));
  FerrumCall registerIssuer(Uint8List who) => _call(_identity, 3, (w) => w.fixed(who));
}

class CredentialCalls {
  FerrumCall issue(CredentialAnchor anchor) => _call(_credential, 0, anchor.encode);
  FerrumCall revoke(String payloadHashHex) =>
      _call(_credential, 1, (w) => w.fixed(hex32(payloadHashHex)));
  FerrumCall setStatus(String payloadHashHex, CredentialStatus status) =>
      _call(_credential, 2, (w) => w.fixed(hex32(payloadHashHex)).u8(status.index));
  FerrumCall logPresentation(String nullifierHex, String commitmentHex) =>
      _call(_credential, 3,
          (w) => w.fixed(hex32(nullifierHex)).fixed(hex32(commitmentHex)));
}

class TaxCalls {
  FerrumCall anchorInvoice(InvoiceAnchor anchor) => _call(_tax, 0, anchor.encode);
  FerrumCall withhold(Did subject, TaxKind kind, FiatAmount amount) =>
      _call(_tax, 1, (w) {
        subject.encode(w);
        w.u8(kind.index);
        amount.encode(w);
      });
  FerrumCall fileObligation(TaxObligation obligation) =>
      _call(_tax, 2, obligation.encode);
  FerrumCall proveBracket(Uint8List proof, AgeProofPublicInputs inputs) =>
      _call(_tax, 3, (w) {
        w.bytes(proof);
        inputs.encode(w);
      });
  FerrumCall settle(Did subject, int slot) => _call(_tax, 4, (w) {
        subject.encode(w);
        w.u64(slot);
      });
  FerrumCall authorizeAudit(String invoiceHex, String viewingKeyCommitmentHex) =>
      _call(_tax, 5,
          (w) => w.fixed(hex32(invoiceHex)).fixed(hex32(viewingKeyCommitmentHex)));
  FerrumCall setBrackets(List<TaxBracket> brackets) =>
      _call(_tax, 6, (w) => w.vec(brackets, (ww, b) => b.encode(ww)));
}

class TreasuryCalls {
  FerrumCall mint(int pool, BigInt amount) =>
      _call(_treasury, 0, (w) => w.u8(pool).u128(amount));
  FerrumCall burn(BigInt amount) => _call(_treasury, 1, (w) => w.u128(amount));
  FerrumCall subsidize(Uint8List who, BigInt amount) =>
      _call(_treasury, 2, (w) => w.fixed(who).u128(amount));
  FerrumCall recordSettlement(String receiptHex, FiatAmount amount) =>
      _call(_treasury, 3, (w) {
        w.fixed(hex32(receiptHex));
        amount.encode(w);
      });
}

class FederationCalls {
  FerrumCall propose(FederationAction action) => _call(_federation, 0, action.encode);
  FerrumCall vote(int id, Vote vote) =>
      _call(_federation, 1, (w) => w.u64(id).u8(vote.index));
  FerrumCall close(int id) => _call(_federation, 2, (w) => w.u64(id));
  FerrumCall setMembership(String member, bool seated) =>
      _call(_federation, 3, (w) => w.asciiFixed(member, 2).boolean(seated));
  FerrumCall setBasket(XsuBasket basket) => _call(_federation, 4, basket.encode);
  FerrumCall mintXsu(String cbdc, BigInt cbdcAmount) =>
      _call(_federation, 5, (w) => w.asciiFixed(cbdc, 3).u128(cbdcAmount));
  FerrumCall redeemXsu(String cbdc, BigInt xsuAmount) =>
      _call(_federation, 6, (w) => w.asciiFixed(cbdc, 3).u128(xsuAmount));
  FerrumCall bookClearing(String to, BigInt amount) =>
      _call(_federation, 7, (w) => w.asciiFixed(to, 2).u128(amount));
  FerrumCall netAndSettle(int window) => _call(_federation, 8, (w) => w.u32(window));
  FerrumCall publishProofOfReserve() => _call(_federation, 9, (w) {});
}

class InteropCalls {
  FerrumCall registerIssuer(TrustRegistryEntry entry) =>
      _call(_interop, 0, entry.encode);
  FerrumCall submitInstruction(ClearingInstruction instr) =>
      _call(_interop, 1, instr.encode);
  FerrumCall verifyFinality(int id, Uint8List finalityProof) =>
      _call(_interop, 2, (w) => w.u64(id).bytes(finalityProof));
  FerrumCall netAndSettle(int window) => _call(_interop, 3, (w) => w.u32(window));
  FerrumCall registerValidator(BigInt bond) =>
      _call(_interop, 4, (w) => w.u128(bond));
  FerrumCall slashValidator(Uint8List who, BigInt amount) =>
      _call(_interop, 5, (w) => w.fixed(who).u128(amount));
  FerrumCall initAuthoritySet(String country, GrandpaAuthoritySet set) =>
      _call(_interop, 6, (w) {
        w.asciiFixed(country, 2);
        set.encode(w);
      });
  FerrumCall rotateAuthoritySet(String country, Uint8List finalityProof) =>
      _call(_interop, 7, (w) => w.asciiFixed(country, 2).bytes(finalityProof));
  FerrumCall registerIssuerVk(String country, String issuerKeyHashHex, Uint8List vk) =>
      _call(_interop, 8,
          (w) => w.asciiFixed(country, 2).fixed(hex32(issuerKeyHashHex)).bytes(vk));
  FerrumCall verifyForeignProof(String country, String issuerKeyHashHex,
          Uint8List proof, AgeProofPublicInputs inputs) =>
      _call(_interop, 9, (w) {
        w.asciiFixed(country, 2).fixed(hex32(issuerKeyHashHex)).bytes(proof);
        inputs.encode(w);
      });
  FerrumCall registerTreaty(String a, String b, TaxTreaty treaty) =>
      _call(_interop, 10, (w) {
        w.asciiFixed(a, 2).asciiFixed(b, 2);
        treaty.encode(w);
      });
  FerrumCall recognizeForeignInvoice(String country, String invoiceHashHex) =>
      _call(_interop, 11,
          (w) => w.asciiFixed(country, 2).fixed(hex32(invoiceHashHex)));
  FerrumCall ossRegister(Did subject, OssRegistration registration) =>
      _call(_interop, 12, (w) {
        subject.encode(w);
        registration.encode(w);
      });
  FerrumCall ossReport(
          Did subject, String to, BigInt amount, String detailCommitmentHex) =>
      _call(_interop, 13, (w) {
        subject.encode(w);
        w.asciiFixed(to, 2).u128(amount).fixed(hex32(detailCommitmentHex));
      });
}
