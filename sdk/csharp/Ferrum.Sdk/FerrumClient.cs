using Substrate.NetApi;
using Substrate.NetApi.Model.Extrinsics;
using Substrate.NetApi.Model.Rpc;
using Substrate.NetApi.Model.Types;

namespace Ferrum.Sdk;

/// <summary>
/// Typed thin wrapper over Substrate.NetApi's <see cref="SubstrateClient"/>.
///
/// The SDK encodes Ferrum call params itself (see <see cref="Calls"/>) and hands
/// the resulting <see cref="Method"/> to Substrate.NetApi, which assembles, signs
/// and submits the extrinsic. Verified against Substrate.NetApi 0.9.x — adjust the
/// submission call if your package version differs.
/// </summary>
public sealed class FerrumClient : IAsyncDisposable
{
    public const string DefaultEndpoint = "ws://127.0.0.1:9944";

    public SubstrateClient Api { get; }
    public IdentityCalls Identity { get; } = new();
    public CredentialCalls Credential { get; } = new();
    public TaxCalls Tax { get; } = new();
    public TreasuryCalls Treasury { get; } = new();
    public FederationCalls Federation { get; } = new();
    public InteropCalls Interop { get; } = new();

    private FerrumClient(SubstrateClient api) => Api = api;

    public static async Task<FerrumClient> ConnectAsync(string endpoint = DefaultEndpoint, CancellationToken token = default)
    {
        var client = new SubstrateClient(new Uri(endpoint), ChargeTransactionPayment.Default());
        await client.ConnectAsync(token);
        return new FerrumClient(client);
    }

    /// <summary>Build a sr25519 account from 32-byte secret + 32-byte public keys.</summary>
    public static Account Account(byte[] secretKey, byte[] publicKey) =>
        Substrate.NetApi.Model.Types.Account.Build(KeyType.Sr25519, secretKey, publicKey);

    /// <summary>Sign and submit a Ferrum call; returns the extrinsic hash.</summary>
    public async Task<string> SignAndSendAsync(FerrumCall call, Account account, uint lifeTime = 64, CancellationToken token = default)
    {
        var method = new Method(call.ModuleIndex, call.CallIndex, call.Parameters);
        var extrinsic = await Api.GetExtrinsicAsync(method, account, ChargeTransactionPayment.Default(), lifeTime, token);
        return await Api.Author.SubmitExtrinsicAsync(Utils.Bytes2HexString(extrinsic.Encode()), token);
    }

    /// <summary>Subscribe to all chain events via the System.Events storage key.</summary>
    public Task<string> SubscribeEventsAsync(Action<string> onChange, CancellationToken token = default) =>
        Api.State.SubscribeStorageAsync(null, (_, change) => onChange(change.ToString() ?? string.Empty), token);

    public async ValueTask DisposeAsync()
    {
        if (Api.IsConnected) await Api.CloseAsync();
        Api.Dispose();
    }
}
