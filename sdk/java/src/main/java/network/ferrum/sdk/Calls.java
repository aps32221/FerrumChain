package network.ferrum.sdk;

import java.math.BigInteger;
import java.util.List;
import java.util.function.Consumer;

import network.ferrum.sdk.FerrumTypes.*;

/** Builders for every Ferrum extrinsic, grouped by pallet. Each returns a
 *  {@link FerrumCall} (pallet index + call index + SCALE params). */
public final class Calls {
    private Calls() {}

    static final int IDENTITY = 10, CREDENTIAL = 11, TAX = 12, TREASURY = 13, FEDERATION = 14, INTEROP = 15;

    /** An encoded call: pallet index, call index and SCALE param bytes. */
    public record FerrumCall(int moduleIndex, int callIndex, byte[] params) {
        /** Full call bytes (module ++ call ++ args). */
        public byte[] encode() {
            byte[] out = new byte[2 + params.length];
            out[0] = (byte) moduleIndex;
            out[1] = (byte) callIndex;
            System.arraycopy(params, 0, out, 2, params.length);
            return out;
        }
    }

    static FerrumCall call(int module, int index, Consumer<ScaleWriter> args) {
        ScaleWriter w = new ScaleWriter();
        args.accept(w);
        return new FerrumCall(module, index, w.toArray());
    }

    public static final class Identity {
        public FerrumCall anchorDid(DidDocument doc) { return call(IDENTITY, 0, doc::encode); }
        public FerrumCall rotateKeys(Did did, List<DidKeyRef> keys) {
            return call(IDENTITY, 1, w -> { did.encode(w); w.vec(keys, (ww, k) -> k.encode(ww)); });
        }
        public FerrumCall updateRevocation(String commitmentHex) { return call(IDENTITY, 2, w -> w.fixed(ScaleWriter.hex32(commitmentHex))); }
        public FerrumCall registerIssuer(byte[] who) { return call(IDENTITY, 3, w -> w.fixed(who)); }
    }

    public static final class Credential {
        public FerrumCall issue(CredentialAnchor anchor) { return call(CREDENTIAL, 0, anchor::encode); }
        public FerrumCall revoke(String payloadHashHex) { return call(CREDENTIAL, 1, w -> w.fixed(ScaleWriter.hex32(payloadHashHex))); }
        public FerrumCall setStatus(String payloadHashHex, CredentialStatus status) {
            return call(CREDENTIAL, 2, w -> w.fixed(ScaleWriter.hex32(payloadHashHex)).u8(status.ordinal()));
        }
        public FerrumCall logPresentation(String nullifierHex, String commitmentHex) {
            return call(CREDENTIAL, 3, w -> w.fixed(ScaleWriter.hex32(nullifierHex)).fixed(ScaleWriter.hex32(commitmentHex)));
        }
    }

    public static final class Tax {
        public FerrumCall anchorInvoice(InvoiceAnchor anchor) { return call(TAX, 0, anchor::encode); }
        public FerrumCall withhold(Did subject, TaxKind kind, FiatAmount amount) {
            return call(TAX, 1, w -> { subject.encode(w); w.u8(kind.ordinal()); amount.encode(w); });
        }
        public FerrumCall fileObligation(TaxObligation obligation) { return call(TAX, 2, obligation::encode); }
        public FerrumCall proveBracket(byte[] proof, AgeProofPublicInputs inputs) {
            return call(TAX, 3, w -> { w.bytes(proof); inputs.encode(w); });
        }
        public FerrumCall settle(Did subject, long slot) { return call(TAX, 4, w -> { subject.encode(w); w.u64(slot); }); }
        public FerrumCall authorizeAudit(String invoiceHex, String viewingKeyCommitmentHex) {
            return call(TAX, 5, w -> w.fixed(ScaleWriter.hex32(invoiceHex)).fixed(ScaleWriter.hex32(viewingKeyCommitmentHex)));
        }
        public FerrumCall setBrackets(List<TaxBracket> brackets) { return call(TAX, 6, w -> w.vec(brackets, (ww, b) -> b.encode(ww))); }
    }

    public static final class Treasury {
        public FerrumCall mint(int pool, BigInteger amount) { return call(TREASURY, 0, w -> w.u8(pool).u128(amount)); }
        public FerrumCall burn(BigInteger amount) { return call(TREASURY, 1, w -> w.u128(amount)); }
        public FerrumCall subsidize(byte[] who, BigInteger amount) { return call(TREASURY, 2, w -> w.fixed(who).u128(amount)); }
        public FerrumCall recordSettlement(String receiptHex, FiatAmount amount) {
            return call(TREASURY, 3, w -> { w.fixed(ScaleWriter.hex32(receiptHex)); amount.encode(w); });
        }
    }

    public static final class Federation {
        public FerrumCall propose(FederationAction action) { return call(FEDERATION, 0, action::encode); }
        public FerrumCall vote(long id, Vote vote) { return call(FEDERATION, 1, w -> w.u64(id).u8(vote.ordinal())); }
        public FerrumCall close(long id) { return call(FEDERATION, 2, w -> w.u64(id)); }
        public FerrumCall setMembership(String member, boolean seated) { return call(FEDERATION, 3, w -> w.asciiFixed(member, 2).bool(seated)); }
        public FerrumCall setBasket(XsuBasket basket) { return call(FEDERATION, 4, basket::encode); }
        public FerrumCall mintXsu(String cbdc, BigInteger cbdcAmount) { return call(FEDERATION, 5, w -> w.asciiFixed(cbdc, 3).u128(cbdcAmount)); }
        public FerrumCall redeemXsu(String cbdc, BigInteger xsuAmount) { return call(FEDERATION, 6, w -> w.asciiFixed(cbdc, 3).u128(xsuAmount)); }
        public FerrumCall bookClearing(String to, BigInteger amount) { return call(FEDERATION, 7, w -> w.asciiFixed(to, 2).u128(amount)); }
        public FerrumCall netAndSettle(long window) { return call(FEDERATION, 8, w -> w.u32(window)); }
        public FerrumCall publishProofOfReserve() { return call(FEDERATION, 9, w -> {}); }
    }

    public static final class Interop {
        public FerrumCall registerIssuer(TrustRegistryEntry entry) { return call(INTEROP, 0, entry::encode); }
        public FerrumCall submitInstruction(ClearingInstruction instr) { return call(INTEROP, 1, instr::encode); }
        public FerrumCall verifyFinality(long id, byte[] finalityProof) { return call(INTEROP, 2, w -> w.u64(id).bytes(finalityProof)); }
        public FerrumCall netAndSettle(long window) { return call(INTEROP, 3, w -> w.u32(window)); }
        public FerrumCall registerValidator(BigInteger bond) { return call(INTEROP, 4, w -> w.u128(bond)); }
        public FerrumCall slashValidator(byte[] who, BigInteger amount) { return call(INTEROP, 5, w -> w.fixed(who).u128(amount)); }
        public FerrumCall initAuthoritySet(String country, GrandpaAuthoritySet set) {
            return call(INTEROP, 6, w -> { w.asciiFixed(country, 2); set.encode(w); });
        }
        public FerrumCall rotateAuthoritySet(String country, byte[] finalityProof) {
            return call(INTEROP, 7, w -> w.asciiFixed(country, 2).bytes(finalityProof));
        }
        public FerrumCall registerIssuerVk(String country, String issuerKeyHashHex, byte[] vk) {
            return call(INTEROP, 8, w -> w.asciiFixed(country, 2).fixed(ScaleWriter.hex32(issuerKeyHashHex)).bytes(vk));
        }
        public FerrumCall verifyForeignProof(String country, String issuerKeyHashHex, byte[] proof, AgeProofPublicInputs inputs) {
            return call(INTEROP, 9, w -> { w.asciiFixed(country, 2).fixed(ScaleWriter.hex32(issuerKeyHashHex)).bytes(proof); inputs.encode(w); });
        }
        public FerrumCall registerTreaty(String a, String b, TaxTreaty treaty) {
            return call(INTEROP, 10, w -> { w.asciiFixed(a, 2).asciiFixed(b, 2); treaty.encode(w); });
        }
        public FerrumCall recognizeForeignInvoice(String country, String invoiceHashHex) {
            return call(INTEROP, 11, w -> w.asciiFixed(country, 2).fixed(ScaleWriter.hex32(invoiceHashHex)));
        }
        public FerrumCall ossRegister(Did subject, OssRegistration registration) {
            return call(INTEROP, 12, w -> { subject.encode(w); registration.encode(w); });
        }
        public FerrumCall ossReport(Did subject, String to, BigInteger amount, String detailCommitmentHex) {
            return call(INTEROP, 13, w -> { subject.encode(w); w.asciiFixed(to, 2).u128(amount).fixed(ScaleWriter.hex32(detailCommitmentHex)); });
        }
    }
}
