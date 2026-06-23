using System.Numerics;

namespace Ferrum.Sdk;

// Enum discriminants match the Rust declaration order in crates/primitives.
public enum KeyKind : byte { Sr25519 = 0, Ed25519 = 1, Bls12_381 = 2 }
public enum CredentialKind : byte { Nationality = 0, Age = 1, Residence = 2, TaxStatus = 3, Other = 4 }
public enum CredentialStatus : byte { Active = 0, Suspended = 1, Revoked = 2, Expired = 3 }
public enum TaxKind : byte { Income = 0, Wage = 1, Interest = 2, ValueAdded = 3, Withholding = 4, Other = 5 }
public enum Vote : byte { Aye = 0, Nay = 1, Abstain = 2 }
public enum XcmStatus : byte { Pending = 0, FinalityVerified = 1, Accepted = 2, Rejected = 3 }
public enum CreditMethod : byte { Credit = 0, Exemption = 1 }

/// <summary>did:fer identifier. Tag/id given as ASCII; bytes are derived.</summary>
public sealed record Did(string ChainTag, byte[] Id)
{
    public static Did Of(string chainTag, string id) => new(chainTag, System.Text.Encoding.ASCII.GetBytes(id));
    public void Encode(ScaleWriter w) { w.AsciiVec(ChainTag); w.Bytes(Id); }
}

public sealed record DidKeyRef(KeyKind Kind, string KeyHashHex)
{
    public void Encode(ScaleWriter w) { w.U8((byte)Kind); w.Fixed(Hex.Bytes32(KeyHashHex)); }
}

public sealed record DidDocument(
    Did Did, byte[] Controller, string DocHashHex, IReadOnlyList<DidKeyRef> Keys,
    string RevocationCommitmentHex, uint AnchoredAt)
{
    public void Encode(ScaleWriter w)
    {
        Did.Encode(w);
        w.Fixed(Controller);
        w.Fixed(Hex.Bytes32(DocHashHex));
        w.Vec(Keys, (ww, k) => k.Encode(ww));
        w.Fixed(Hex.Bytes32(RevocationCommitmentHex));
        w.U32(AnchoredAt);
    }
}

public sealed record FiatAmount(string Currency, BigInteger MinorUnits)
{
    public void Encode(ScaleWriter w) { w.AsciiFixed(Currency, 3); w.U128(MinorUnits); }
}

public sealed record CredentialAnchor(
    Did Subject, byte[] Issuer, CredentialKind Kind, string PayloadHashHex,
    CredentialStatus Status, ulong? ExpiresAt)
{
    public void Encode(ScaleWriter w)
    {
        Subject.Encode(w);
        w.Fixed(Issuer);
        w.U8((byte)Kind);
        w.Fixed(Hex.Bytes32(PayloadHashHex));
        w.U8((byte)Status);
        w.Option(ExpiresAt, (ww, v) => ww.U64(v));
    }
}

public sealed record TaxBracket(byte Index, double Rate)
{
    public void Encode(ScaleWriter w) { w.U8(Index); w.U32(Hex.Perbill(Rate)); }
}

public sealed record InvoiceAnchor(string InvoiceHashHex, byte[] Issuer, TaxKind Kind, ulong AnchoredAt)
{
    public void Encode(ScaleWriter w)
    {
        w.Fixed(Hex.Bytes32(InvoiceHashHex));
        w.Fixed(Issuer);
        w.U8((byte)Kind);
        w.U64(AnchoredAt);
    }
}

public sealed record TaxObligation(Did Subject, TaxKind Kind, FiatAmount AmountDue, string DetailCommitmentHex, bool Settled)
{
    public void Encode(ScaleWriter w)
    {
        Subject.Encode(w);
        w.U8((byte)Kind);
        AmountDue.Encode(w);
        w.Fixed(Hex.Bytes32(DetailCommitmentHex));
        w.Bool(Settled);
    }
}

public sealed record AgeProofPublicInputs(string IssuerCommitmentHex, uint Threshold, string NullifierHex)
{
    public void Encode(ScaleWriter w)
    {
        w.Fixed(Hex.Bytes32(IssuerCommitmentHex));
        w.U32(Threshold);
        w.Fixed(Hex.Bytes32(NullifierHex));
    }
}

public sealed record BasketEntry(string Cbdc, double Weight)
{
    public void Encode(ScaleWriter w) { w.AsciiFixed(Cbdc, 3); w.U32(Hex.Perbill(Weight)); }
}

public sealed record XsuBasket(IReadOnlyList<BasketEntry> Entries, uint Version)
{
    public void Encode(ScaleWriter w) { w.Vec(Entries, (ww, e) => e.Encode(ww)); w.U32(Version); }
}

public abstract record FederationAction
{
    public abstract void Encode(ScaleWriter w);

    public sealed record SetParameter(string Key, BigInteger Value) : FederationAction
    { public override void Encode(ScaleWriter w) { w.U8(0); w.AsciiVec(Key); w.U128(Value); } }
    public sealed record AdmitMember(string Member) : FederationAction
    { public override void Encode(ScaleWriter w) { w.U8(1); w.AsciiFixed(Member, 2); } }
    public sealed record RemoveMember(string Member) : FederationAction
    { public override void Encode(ScaleWriter w) { w.U8(2); w.AsciiFixed(Member, 2); } }
    public sealed record Reweight(XsuBasket Basket) : FederationAction
    { public override void Encode(ScaleWriter w) { w.U8(3); Basket.Encode(w); } }
    public sealed record SuspendMember(string Member) : FederationAction
    { public override void Encode(ScaleWriter w) { w.U8(4); w.AsciiFixed(Member, 2); } }
    public sealed record RuntimeUpgrade(string CodeHashHex) : FederationAction
    { public override void Encode(ScaleWriter w) { w.U8(5); w.Fixed(Hex.Bytes32(CodeHashHex)); } }
}

public sealed record TrustRegistryEntry(string Country, string IssuerKeyHashHex, string Scope, bool Active)
{
    public void Encode(ScaleWriter w)
    {
        w.AsciiFixed(Country, 2);
        w.Fixed(Hex.Bytes32(IssuerKeyHashHex));
        w.AsciiVec(Scope);
        w.Bool(Active);
    }
}

public sealed record ClearingInstruction(string From, string To, BigInteger Amount, string DetailCommitmentHex, XcmStatus Status = XcmStatus.Pending)
{
    public void Encode(ScaleWriter w)
    {
        w.AsciiFixed(From, 2);
        w.AsciiFixed(To, 2);
        w.U128(Amount);
        w.Fixed(Hex.Bytes32(DetailCommitmentHex));
        w.U8((byte)Status);
    }
}

public sealed record TaxTreaty(double WithholdingCap, CreditMethod Method, bool Active)
{
    public void Encode(ScaleWriter w) { w.U32(Hex.Perbill(WithholdingCap)); w.U8((byte)Method); w.Bool(Active); }
}

public sealed record OssRegistration(string Home, string VatIdCommitmentHex, bool Active)
{
    public void Encode(ScaleWriter w) { w.AsciiFixed(Home, 2); w.Fixed(Hex.Bytes32(VatIdCommitmentHex)); w.Bool(Active); }
}

public sealed record GrandpaAuthority(string IdHex, ulong Weight)
{
    public void Encode(ScaleWriter w) { w.Fixed(Hex.Bytes32(IdHex)); w.U64(Weight); }
}

public sealed record GrandpaAuthoritySet(IReadOnlyList<GrandpaAuthority> Authorities, ulong SetId)
{
    public void Encode(ScaleWriter w) { w.Vec(Authorities, (ww, a) => a.Encode(ww)); w.U64(SetId); }
}

/// <summary>Treasury allocation pool ids (§08).</summary>
public static class Pools
{
    public const byte Staking = 0, Treasury = 1, Subsidy = 2, Dev = 3, Ecosystem = 4;
}
