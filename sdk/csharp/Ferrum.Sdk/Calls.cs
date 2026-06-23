using System.Numerics;

namespace Ferrum.Sdk;

/// <summary>An encoded call: pallet index, call index and SCALE-encoded params.</summary>
public readonly record struct FerrumCall(byte ModuleIndex, byte CallIndex, byte[] Parameters)
{
    /// <summary>Full call bytes as the runtime expects them (module ++ call ++ args).</summary>
    public byte[] Encode()
    {
        var bytes = new byte[2 + Parameters.Length];
        bytes[0] = ModuleIndex;
        bytes[1] = CallIndex;
        Array.Copy(Parameters, 0, bytes, 2, Parameters.Length);
        return bytes;
    }
}

internal static class Mod
{
    public const byte Identity = 10, Credential = 11, Tax = 12, Treasury = 13, Federation = 14, Interop = 15;
}

internal static class Build
{
    public static FerrumCall Call(byte module, byte index, Action<ScaleWriter> args)
    {
        var w = new ScaleWriter();
        args(w);
        return new FerrumCall(module, index, w.ToArray());
    }
}

public sealed class IdentityCalls
{
    public FerrumCall AnchorDid(DidDocument doc) => Build.Call(Mod.Identity, 0, w => doc.Encode(w));
    public FerrumCall RotateKeys(Did did, IReadOnlyList<DidKeyRef> keys) =>
        Build.Call(Mod.Identity, 1, w => { did.Encode(w); w.Vec(keys, (ww, k) => k.Encode(ww)); });
    public FerrumCall UpdateRevocation(string commitmentHex) => Build.Call(Mod.Identity, 2, w => w.Fixed(Hex.Bytes32(commitmentHex)));
    public FerrumCall RegisterIssuer(byte[] who) => Build.Call(Mod.Identity, 3, w => w.Fixed(who));
}

public sealed class CredentialCalls
{
    public FerrumCall Issue(CredentialAnchor anchor) => Build.Call(Mod.Credential, 0, w => anchor.Encode(w));
    public FerrumCall Revoke(string payloadHashHex) => Build.Call(Mod.Credential, 1, w => w.Fixed(Hex.Bytes32(payloadHashHex)));
    public FerrumCall SetStatus(string payloadHashHex, CredentialStatus status) =>
        Build.Call(Mod.Credential, 2, w => { w.Fixed(Hex.Bytes32(payloadHashHex)); w.U8((byte)status); });
    public FerrumCall LogPresentation(string nullifierHex, string commitmentHex) =>
        Build.Call(Mod.Credential, 3, w => { w.Fixed(Hex.Bytes32(nullifierHex)); w.Fixed(Hex.Bytes32(commitmentHex)); });
}

public sealed class TaxCalls
{
    public FerrumCall AnchorInvoice(InvoiceAnchor anchor) => Build.Call(Mod.Tax, 0, w => anchor.Encode(w));
    public FerrumCall Withhold(Did subject, TaxKind kind, FiatAmount amount) =>
        Build.Call(Mod.Tax, 1, w => { subject.Encode(w); w.U8((byte)kind); amount.Encode(w); });
    public FerrumCall FileObligation(TaxObligation obligation) => Build.Call(Mod.Tax, 2, w => obligation.Encode(w));
    public FerrumCall ProveBracket(byte[] proof, AgeProofPublicInputs inputs) =>
        Build.Call(Mod.Tax, 3, w => { w.Bytes(proof); inputs.Encode(w); });
    public FerrumCall Settle(Did subject, ulong slot) => Build.Call(Mod.Tax, 4, w => { subject.Encode(w); w.U64(slot); });
    public FerrumCall AuthorizeAudit(string invoiceHex, string viewingKeyCommitmentHex) =>
        Build.Call(Mod.Tax, 5, w => { w.Fixed(Hex.Bytes32(invoiceHex)); w.Fixed(Hex.Bytes32(viewingKeyCommitmentHex)); });
    public FerrumCall SetBrackets(IReadOnlyList<TaxBracket> brackets) =>
        Build.Call(Mod.Tax, 6, w => w.Vec(brackets, (ww, b) => b.Encode(ww)));
}

public sealed class TreasuryCalls
{
    public FerrumCall Mint(byte pool, BigInteger amount) => Build.Call(Mod.Treasury, 0, w => { w.U8(pool); w.U128(amount); });
    public FerrumCall Burn(BigInteger amount) => Build.Call(Mod.Treasury, 1, w => w.U128(amount));
    public FerrumCall Subsidize(byte[] who, BigInteger amount) => Build.Call(Mod.Treasury, 2, w => { w.Fixed(who); w.U128(amount); });
    public FerrumCall RecordSettlement(string receiptHex, FiatAmount amount) =>
        Build.Call(Mod.Treasury, 3, w => { w.Fixed(Hex.Bytes32(receiptHex)); amount.Encode(w); });
}

public sealed class FederationCalls
{
    public FerrumCall Propose(FederationAction action) => Build.Call(Mod.Federation, 0, w => action.Encode(w));
    public FerrumCall Vote(ulong id, Vote vote) => Build.Call(Mod.Federation, 1, w => { w.U64(id); w.U8((byte)vote); });
    public FerrumCall Close(ulong id) => Build.Call(Mod.Federation, 2, w => w.U64(id));
    public FerrumCall SetMembership(string member, bool seated) => Build.Call(Mod.Federation, 3, w => { w.AsciiFixed(member, 2); w.Bool(seated); });
    public FerrumCall SetBasket(XsuBasket basket) => Build.Call(Mod.Federation, 4, w => basket.Encode(w));
    public FerrumCall MintXsu(string cbdc, BigInteger cbdcAmount) => Build.Call(Mod.Federation, 5, w => { w.AsciiFixed(cbdc, 3); w.U128(cbdcAmount); });
    public FerrumCall RedeemXsu(string cbdc, BigInteger xsuAmount) => Build.Call(Mod.Federation, 6, w => { w.AsciiFixed(cbdc, 3); w.U128(xsuAmount); });
    public FerrumCall BookClearing(string to, BigInteger amount) => Build.Call(Mod.Federation, 7, w => { w.AsciiFixed(to, 2); w.U128(amount); });
    public FerrumCall NetAndSettle(uint window) => Build.Call(Mod.Federation, 8, w => w.U32(window));
    public FerrumCall PublishProofOfReserve() => Build.Call(Mod.Federation, 9, _ => { });
}

public sealed class InteropCalls
{
    public FerrumCall RegisterIssuer(TrustRegistryEntry entry) => Build.Call(Mod.Interop, 0, w => entry.Encode(w));
    public FerrumCall SubmitInstruction(ClearingInstruction instr) => Build.Call(Mod.Interop, 1, w => instr.Encode(w));
    public FerrumCall VerifyFinality(ulong id, byte[] finalityProof) => Build.Call(Mod.Interop, 2, w => { w.U64(id); w.Bytes(finalityProof); });
    public FerrumCall NetAndSettle(uint window) => Build.Call(Mod.Interop, 3, w => w.U32(window));
    public FerrumCall RegisterValidator(BigInteger bond) => Build.Call(Mod.Interop, 4, w => w.U128(bond));
    public FerrumCall SlashValidator(byte[] who, BigInteger amount) => Build.Call(Mod.Interop, 5, w => { w.Fixed(who); w.U128(amount); });
    public FerrumCall InitAuthoritySet(string country, GrandpaAuthoritySet set) => Build.Call(Mod.Interop, 6, w => { w.AsciiFixed(country, 2); set.Encode(w); });
    public FerrumCall RotateAuthoritySet(string country, byte[] finalityProof) => Build.Call(Mod.Interop, 7, w => { w.AsciiFixed(country, 2); w.Bytes(finalityProof); });
    public FerrumCall RegisterIssuerVk(string country, string issuerKeyHashHex, byte[] vk) =>
        Build.Call(Mod.Interop, 8, w => { w.AsciiFixed(country, 2); w.Fixed(Hex.Bytes32(issuerKeyHashHex)); w.Bytes(vk); });
    public FerrumCall VerifyForeignProof(string country, string issuerKeyHashHex, byte[] proof, AgeProofPublicInputs inputs) =>
        Build.Call(Mod.Interop, 9, w => { w.AsciiFixed(country, 2); w.Fixed(Hex.Bytes32(issuerKeyHashHex)); w.Bytes(proof); inputs.Encode(w); });
    public FerrumCall RegisterTreaty(string a, string b, TaxTreaty treaty) =>
        Build.Call(Mod.Interop, 10, w => { w.AsciiFixed(a, 2); w.AsciiFixed(b, 2); treaty.Encode(w); });
    public FerrumCall RecognizeForeignInvoice(string country, string invoiceHashHex) =>
        Build.Call(Mod.Interop, 11, w => { w.AsciiFixed(country, 2); w.Fixed(Hex.Bytes32(invoiceHashHex)); });
    public FerrumCall OssRegister(Did subject, OssRegistration registration) =>
        Build.Call(Mod.Interop, 12, w => { subject.Encode(w); registration.Encode(w); });
    public FerrumCall OssReport(Did subject, string to, BigInteger amount, string detailCommitmentHex) =>
        Build.Call(Mod.Interop, 13, w => { subject.Encode(w); w.AsciiFixed(to, 2); w.U128(amount); w.Fixed(Hex.Bytes32(detailCommitmentHex)); });
}
