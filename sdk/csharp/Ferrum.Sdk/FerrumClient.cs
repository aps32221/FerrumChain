using System.Numerics;
using Substrate.NetApi;
using Substrate.NetApi.Model.Extrinsics;
using Substrate.NetApi.Model.Rpc;
using Substrate.NetApi.Model.Types;

namespace Ferrum.Sdk;

/// <summary>
/// Typed thin wrapper over Substrate.NetApi's <see cref="SubstrateClient"/>.
///
/// The SDK encodes Ferrum call params itself (see <see cref="Calls"/>) and assembles
/// the signed extrinsic by hand to match the runtime's 8-field SignedExtra. We do NOT
/// delegate to Substrate.NetApi's extrinsic builder: that version appends a
/// CheckMetadataHash extension byte the Ferrum runtime does not carry, which shifts the
/// call and traps validate_transaction ("Bad input data").
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
        // useMetaData + standardSubstrate so RuntimeVersion and GenesisHash are populated.
        await client.ConnectAsync(true, true, token);
        return new FerrumClient(client);
    }

    /// <summary>Build a sr25519 account from a 64-byte expanded secret + 32-byte public key.</summary>
    public static Account Account(byte[] secretKey, byte[] publicKey) =>
        Substrate.NetApi.Model.Types.Account.Build(KeyType.Sr25519, secretKey, publicKey);

    /// <summary>
    /// Build a sr25519 account from a 32-byte secret seed (the "Secret seed" printed by
    /// <c>ferrum-node key generate</c>). The seed is expanded to the 64-byte key Schnorrkel
    /// signs with and the public key is derived — passing the raw seed to <see cref="Account"/>
    /// throws SignatureError::ByteLengthError.
    /// </summary>
    public static Account AccountFromSeed(byte[] seed) =>
        Substrate.NetApi.Model.Types.Account.FromSeed(KeyType.Sr25519, seed);

    /// <summary>Sign and submit a Ferrum call; returns the extrinsic hash.</summary>
    /// <remarks>
    /// Assembles a v4 signed extrinsic with the runtime's 8-field SignedExtra
    /// (immortal era, zero tip). <paramref name="lifeTime"/> is accepted for API
    /// compatibility but unused — the era is immortal, so the era checkpoint hash is
    /// the genesis hash.
    /// </remarks>
    public async Task<string> SignAndSendAsync(FerrumCall call, Account account, uint lifeTime = 64, CancellationToken token = default)
    {
        _ = lifeTime;
        var callBytes = call.Encode();

        var rv = Api.RuntimeVersion;
        var genesis = Api.GenesisHash.Bytes;
        var nonce = Convert.ToUInt32(await Api.System.AccountNextIndexAsync(account.Value, token));

        // extra (the signed part): immortal era ++ compact(nonce) ++ compact(tip = 0)
        var extra = new ScaleWriter().U8(0).Compact(nonce).Compact(BigInteger.Zero).ToArray();
        // additional signed: specVersion ++ txVersion ++ genesis ++ era checkpoint (== genesis)
        var additional = new ScaleWriter()
            .U32(rv.SpecVersion).U32(rv.TransactionVersion).Fixed(genesis).Fixed(genesis).ToArray();

        var payload = new ScaleWriter().Fixed(callBytes).Fixed(extra).Fixed(additional).ToArray();
        if (payload.Length > 256) payload = HashExtension.Blake2(payload, 256, null); // sign the hash for long payloads
        var signature = await account.SignAsync(payload);

        var body = new ScaleWriter()
            .U8(0x84).U8(0x00).Fixed(account.Bytes) // v4 signed, MultiAddress::Id(public key)
            .U8(0x01).Fixed(signature)              // MultiSignature::Sr25519
            .Fixed(extra).Fixed(callBytes)
            .ToArray();

        var framed = new ScaleWriter().Bytes(body).ToArray(); // compact length prefix ++ body
        var hash = await Api.Author.SubmitExtrinsicAsync(Utils.Bytes2HexString(framed), token);
        return Utils.Bytes2HexString(hash.Bytes);
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
