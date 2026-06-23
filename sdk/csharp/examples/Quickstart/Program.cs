// Quickstart against a local dev node:  ./target/release/ferrum-node --dev
//
//     dotnet run --project examples/Quickstart
//
// Anchors a DID and files a tax obligation. Supply your own accredited-issuer
// account keys (the dev chain's //Alice is the sudo/governance key). On --dev,
// issuer accreditation is done once via a sudo-wrapped Identity.registerIssuer.

using System.Numerics;
using System.Security.Cryptography;
using Ferrum.Sdk;

static string Commit(string s)
{
    // BLAKE2b-256 commitment computed off-chain — never the document itself.
    using var h = IncrementalHash.CreateHash(HashAlgorithmName.SHA256); // placeholder; use a BLAKE2b lib in production
    h.AppendData(System.Text.Encoding.UTF8.GetBytes(s));
    return "0x" + Convert.ToHexString(h.GetHashAndReset()).ToLowerInvariant();
}

await using var ferrum = await FerrumClient.ConnectAsync("ws://127.0.0.1:9944");

// Provide your issuer account keys (32-byte secret + 32-byte public).
byte[] secret = Hex.Decode(Environment.GetEnvironmentVariable("FERRUM_SECRET") ?? new string('0', 64));
byte[] pub = Hex.Decode(Environment.GetEnvironmentVariable("FERRUM_PUBLIC") ?? new string('0', 64));
var issuer = FerrumClient.Account(secret, pub);

var subject = Did.Of("tw", "citizen-0001");

var anchor = ferrum.Identity.AnchorDid(new DidDocument(
    Did: subject,
    Controller: pub,
    DocHashHex: Commit("off-chain DID document for citizen #1"),
    Keys: new[] { new DidKeyRef(KeyKind.Sr25519, Commit("device-key")) },
    RevocationCommitmentHex: Commit("rev-acc-0"),
    AnchoredAt: 0));
Console.WriteLine("anchor_did hash: " + await ferrum.SignAndSendAsync(anchor, issuer));

var obligation = ferrum.Tax.FileObligation(new TaxObligation(
    Subject: subject,
    Kind: TaxKind.Income,
    AmountDue: new FiatAmount("TWD", new BigInteger(1234500)),
    DetailCommitmentHex: Commit("encrypted return detail"),
    Settled: false));
Console.WriteLine("file_obligation hash: " + await ferrum.SignAndSendAsync(obligation, issuer));
