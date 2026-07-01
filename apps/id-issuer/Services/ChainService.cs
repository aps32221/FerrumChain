#if FERRUM_CHAIN
using Ferrum.Sdk;

namespace IdIssuer.Services;

/// <summary>
/// Optional on-chain step: anchors the issued DID via Identity.anchor_did using the
/// Ferrum C# SDK. The signing account needs FER to pay the transaction fee.
/// Compiled only when the project is built with -p:EnableChain=true.
/// </summary>
public static class ChainService
{
    public const string DefaultEndpoint = FerrumClient.DefaultEndpoint;

    public static async Task<string> AnchorAsync(string endpoint, string seedHex, IdCard card)
    {
        var seed = Hex.Bytes32(seedHex); // 32-byte "Secret seed"

        await using var ferrum = await FerrumClient.ConnectAsync(endpoint);
        var account = FerrumClient.AccountFromSeed(seed); // expands seed -> 64-byte sr25519 key
        var pub = account.Bytes;                          // 32-byte public, derived from the seed

        var doc = new DidDocument(
            Did: Did.Of(card.ChainTag, card.NationalId),
            Controller: pub,
            DocHashHex: card.DocHashHex,
            Keys: new[] { new DidKeyRef(KeyKind.Sr25519, card.DocHashHex) },
            RevocationCommitmentHex: card.DocHashHex,
            AnchoredAt: 0); // the runtime stamps the real block height

        try
        {
            return await ferrum.SignAndSendAsync(ferrum.Identity.AnchorDid(doc), account);
        }
        catch (Exception ex)
        {
            throw new InvalidOperationException(Explain(ex), ex);
        }
    }

    /// <summary>Surface the JSON-RPC error detail the node returns (otherwise hidden behind "Invalid Transaction").</summary>
    private static string Explain(Exception ex)
    {
        // StreamJsonRpc.RemoteInvocationException carries the node's reason in ErrorData.
        var data = ex.GetType().GetProperties()
            .FirstOrDefault(p => p.Name == "ErrorData")?.GetValue(ex)?.ToString();
        var detail = string.IsNullOrWhiteSpace(data) ? ex.Message : $"{ex.Message} — {data}";

        if (detail.Contains("pay some fees", StringComparison.OrdinalIgnoreCase) ||
            detail.Contains("balance too low", StringComparison.OrdinalIgnoreCase))
            detail += "\n此帳戶沒有 FER 可付手續費。請改用已撥款的 //Alice，或先為此帳戶充值 FER。";

        return detail;
    }
}
#endif
