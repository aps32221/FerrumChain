using System.Security.Cryptography;
using System.Text;
using System.Text.Json;

namespace IdIssuer.Services;

/// <summary>
/// An issued Ferrum digital identity card. The on-chain record only ever holds a
/// 32-byte commitment (<see cref="DocHashHex"/>); the personal fields stay with the
/// holder and are re-derivable from the QR payload for offline verification.
/// </summary>
public sealed record IdCard(
    string Name,
    string NationalId,
    string BirthDate,
    string Nationality,
    string ChainTag,
    string Issuer,
    string Did,
    string DocHashHex,
    string IssuedAt)
{
    public static IdCard Issue(
        string name, string nationalId, string birthDate,
        string nationality, string chainTag, string issuer)
    {
        var did = $"did:fer:{chainTag}:{nationalId}";
        var issuedAt = DateTime.Now.ToString("yyyy-MM-dd HH:mm:ss");

        // BLAKE2b-256 in production; SHA-256 here mirrors the SDK quickstart placeholder.
        var canonical = string.Join("|", did, name, nationalId, birthDate, nationality, issuer, issuedAt);
        var hash = SHA256.HashData(Encoding.UTF8.GetBytes(canonical));
        var docHash = "0x" + Convert.ToHexString(hash).ToLowerInvariant();

        return new IdCard(name, nationalId, birthDate, nationality, chainTag, issuer, did, docHash, issuedAt);
    }

    /// <summary>Compact JSON a verifier app can scan and re-hash against the chain.</summary>
    public string ToQrPayload() => JsonSerializer.Serialize(new
    {
        v = 1,
        did = Did,
        name = Name,
        id = NationalId,
        dob = BirthDate,
        nat = Nationality,
        issuer = Issuer,
        docHash = DocHashHex,
        issuedAt = IssuedAt,
    });
}
