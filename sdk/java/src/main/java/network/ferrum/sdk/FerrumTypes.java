package network.ferrum.sdk;

import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.util.List;

/** Ferrum value types and their SCALE encoders. Enum ordinals match the Rust
 *  declaration order in crates/primitives (i.e. the on-chain discriminants). */
public final class FerrumTypes {
    private FerrumTypes() {}

    public enum KeyKind { Sr25519, Ed25519, Bls12_381 }
    public enum CredentialKind { Nationality, Age, Residence, TaxStatus, Other }
    public enum CredentialStatus { Active, Suspended, Revoked, Expired }
    public enum TaxKind { Income, Wage, Interest, ValueAdded, Withholding, Other }
    public enum Vote { Aye, Nay, Abstain }
    public enum XcmStatus { Pending, FinalityVerified, Accepted, Rejected }
    public enum CreditMethod { Credit, Exemption }

    public record Did(String chainTag, byte[] id) {
        public static Did of(String chainTag, String id) { return new Did(chainTag, id.getBytes(StandardCharsets.US_ASCII)); }
        public void encode(ScaleWriter w) { w.asciiVec(chainTag).bytes(id); }
    }

    public record DidKeyRef(KeyKind kind, String keyHashHex) {
        public void encode(ScaleWriter w) { w.u8(kind.ordinal()).fixed(ScaleWriter.hex32(keyHashHex)); }
    }

    public record DidDocument(Did did, byte[] controller, String docHashHex, List<DidKeyRef> keys,
                              String revocationCommitmentHex, long anchoredAt) {
        public void encode(ScaleWriter w) {
            did.encode(w);
            w.fixed(controller).fixed(ScaleWriter.hex32(docHashHex));
            w.vec(keys, (ww, k) -> k.encode(ww));
            w.fixed(ScaleWriter.hex32(revocationCommitmentHex)).u32(anchoredAt);
        }
    }

    public record FiatAmount(String currency, BigInteger minorUnits) {
        public void encode(ScaleWriter w) { w.asciiFixed(currency, 3).u128(minorUnits); }
    }

    public record CredentialAnchor(Did subject, byte[] issuer, CredentialKind kind, String payloadHashHex,
                                   CredentialStatus status, Long expiresAt) {
        public void encode(ScaleWriter w) {
            subject.encode(w);
            w.fixed(issuer).u8(kind.ordinal()).fixed(ScaleWriter.hex32(payloadHashHex)).u8(status.ordinal());
            w.option(expiresAt, (ww, v) -> ww.u64(v));
        }
    }

    public record TaxBracket(int index, double rate) {
        public void encode(ScaleWriter w) { w.u8(index).u32(ScaleWriter.perbill(rate)); }
    }

    public record InvoiceAnchor(String invoiceHashHex, byte[] issuer, TaxKind kind, long anchoredAt) {
        public void encode(ScaleWriter w) {
            w.fixed(ScaleWriter.hex32(invoiceHashHex)).fixed(issuer).u8(kind.ordinal()).u64(anchoredAt);
        }
    }

    public record TaxObligation(Did subject, TaxKind kind, FiatAmount amountDue, String detailCommitmentHex, boolean settled) {
        public void encode(ScaleWriter w) {
            subject.encode(w);
            w.u8(kind.ordinal());
            amountDue.encode(w);
            w.fixed(ScaleWriter.hex32(detailCommitmentHex)).bool(settled);
        }
    }

    public record AgeProofPublicInputs(String issuerCommitmentHex, long threshold, String nullifierHex) {
        public void encode(ScaleWriter w) {
            w.fixed(ScaleWriter.hex32(issuerCommitmentHex)).u32(threshold).fixed(ScaleWriter.hex32(nullifierHex));
        }
    }

    public record BasketEntry(String cbdc, double weight) {
        public void encode(ScaleWriter w) { w.asciiFixed(cbdc, 3).u32(ScaleWriter.perbill(weight)); }
    }

    public record XsuBasket(List<BasketEntry> entries, long version) {
        public void encode(ScaleWriter w) { w.vec(entries, (ww, e) -> e.encode(ww)).u32(version); }
    }

    public sealed interface FederationAction {
        void encode(ScaleWriter w);
        record SetParameter(String key, BigInteger value) implements FederationAction {
            public void encode(ScaleWriter w) { w.u8(0).asciiVec(key).u128(value); }
        }
        record AdmitMember(String member) implements FederationAction {
            public void encode(ScaleWriter w) { w.u8(1).asciiFixed(member, 2); }
        }
        record RemoveMember(String member) implements FederationAction {
            public void encode(ScaleWriter w) { w.u8(2).asciiFixed(member, 2); }
        }
        record Reweight(XsuBasket basket) implements FederationAction {
            public void encode(ScaleWriter w) { w.u8(3); basket.encode(w); }
        }
        record SuspendMember(String member) implements FederationAction {
            public void encode(ScaleWriter w) { w.u8(4).asciiFixed(member, 2); }
        }
        record RuntimeUpgrade(String codeHashHex) implements FederationAction {
            public void encode(ScaleWriter w) { w.u8(5).fixed(ScaleWriter.hex32(codeHashHex)); }
        }
    }

    public record TrustRegistryEntry(String country, String issuerKeyHashHex, String scope, boolean active) {
        public void encode(ScaleWriter w) {
            w.asciiFixed(country, 2).fixed(ScaleWriter.hex32(issuerKeyHashHex)).asciiVec(scope).bool(active);
        }
    }

    public record ClearingInstruction(String from, String to, BigInteger amount, String detailCommitmentHex, XcmStatus status) {
        public ClearingInstruction(String from, String to, BigInteger amount, String detailCommitmentHex) {
            this(from, to, amount, detailCommitmentHex, XcmStatus.Pending);
        }
        public void encode(ScaleWriter w) {
            w.asciiFixed(from, 2).asciiFixed(to, 2).u128(amount).fixed(ScaleWriter.hex32(detailCommitmentHex)).u8(status.ordinal());
        }
    }

    public record TaxTreaty(double withholdingCap, CreditMethod method, boolean active) {
        public void encode(ScaleWriter w) { w.u32(ScaleWriter.perbill(withholdingCap)).u8(method.ordinal()).bool(active); }
    }

    public record OssRegistration(String home, String vatIdCommitmentHex, boolean active) {
        public void encode(ScaleWriter w) { w.asciiFixed(home, 2).fixed(ScaleWriter.hex32(vatIdCommitmentHex)).bool(active); }
    }

    public record GrandpaAuthority(String idHex, long weight) {
        public void encode(ScaleWriter w) { w.fixed(ScaleWriter.hex32(idHex)).u64(weight); }
    }

    public record GrandpaAuthoritySet(List<GrandpaAuthority> authorities, long setId) {
        public void encode(ScaleWriter w) { w.vec(authorities, (ww, a) -> a.encode(ww)).u64(setId); }
    }

    /** Treasury allocation pool ids (§08). */
    public static final int POOL_STAKING = 0, POOL_TREASURY = 1, POOL_SUBSIDY = 2, POOL_DEV = 3, POOL_ECOSYSTEM = 4;
}
