package network.ferrum.sdk;

import java.math.BigInteger;

import org.bouncycastle.crypto.digests.Blake2bDigest;

import io.emeraldpay.polkaj.api.PolkadotApi;
import io.emeraldpay.polkaj.api.RpcCall;
import io.emeraldpay.polkaj.api.StandardCommands;
import io.emeraldpay.polkaj.apiws.JavaHttpSubscriptionAdapter;
import io.emeraldpay.polkaj.schnorrkel.Schnorrkel;
import io.emeraldpay.polkaj.types.Address;
import io.emeraldpay.polkaj.types.Hash256;
import io.emeraldpay.polkaj.ss58.SS58Type;

import network.ferrum.sdk.Calls.FerrumCall;

/**
 * Typed thin wrapper over polkaj for the Ferrum sovereign blockchain.
 *
 * polkaj provides the WebSocket transport, sr25519 (schnorrkel) signing and SS58
 * codec; this client SCALE-encodes the call params ({@link Calls}) and assembles a
 * v4 signed extrinsic with the runtime's 8-field SignedExtra (immortal era, zero
 * tip), then submits it via {@code author_submitExtrinsic}.
 */
public final class FerrumClient implements AutoCloseable {
    public static final String DEFAULT_ENDPOINT = "ws://127.0.0.1:9944";

    public final Calls.Identity identity = new Calls.Identity();
    public final Calls.Credential credential = new Calls.Credential();
    public final Calls.Tax tax = new Calls.Tax();
    public final Calls.Treasury treasury = new Calls.Treasury();
    public final Calls.Federation federation = new Calls.Federation();
    public final Calls.Interop interop = new Calls.Interop();

    private final PolkadotApi api;
    private final JavaHttpSubscriptionAdapter adapter;

    private FerrumClient(PolkadotApi api, JavaHttpSubscriptionAdapter adapter) {
        this.api = api;
        this.adapter = adapter;
    }

    public PolkadotApi raw() { return api; }

    public static FerrumClient connect(String endpoint) throws Exception {
        var adapter = JavaHttpSubscriptionAdapter.newBuilder().connectTo(endpoint).build();
        var api = PolkadotApi.newBuilder().subscriptionAdapter(adapter).build();
        adapter.connect().get();
        return new FerrumClient(api, adapter);
    }

    /** Build a sr25519 keypair from a 32-byte seed (e.g. a wallet seed). */
    public static Schnorrkel.KeyPair keypairFromSeed(byte[] seed32) throws Exception {
        return Schnorrkel.getInstance().generateKeyPairFromSeed(seed32);
    }

    /** Sign and submit a Ferrum call; returns the extrinsic hash. */
    public Hash256 signAndSend(FerrumCall call, Schnorrkel.KeyPair signer) throws Exception {
        byte[] pub = signer.getPublicKey();
        byte[] callBytes = call.encode();

        var rv = api.execute(StandardCommands.getInstance().getRuntimeVersion()).get();
        Hash256 genesis = api.execute(StandardCommands.getInstance().getBlockHash(0)).get();
        String ss58 = new Address(SS58Type.Network.SUBSTRATE, pub).toString();
        Integer nonce = api.execute(RpcCall.create(Integer.class, "system_accountNextIndex", ss58)).get();

        byte[] extra = new ScaleWriter()
                .u8(0)                                   // CheckEra: immortal
                .compact(BigInteger.valueOf(nonce))      // CheckNonce
                .compact(BigInteger.ZERO)                // ChargeTransactionPayment tip
                .toArray();
        byte[] additional = new ScaleWriter()
                .u32(rv.getSpecVersion())                // CheckSpecVersion
                .u32(rv.getTransactionVersion())         // CheckTxVersion
                .fixed(genesis.getBytes())               // CheckGenesis
                .fixed(genesis.getBytes())               // CheckEra (immortal -> genesis)
                .toArray();

        byte[] payload = concat(callBytes, extra, additional);
        if (payload.length > 256) payload = blake2b256(payload);
        byte[] sig = Schnorrkel.getInstance().sign(payload, signer);

        byte[] body = new ScaleWriter()
                .u8(0x84)            // v4, signed
                .u8(0x00).fixed(pub) // MultiAddress::Id(account)
                .u8(0x01).fixed(sig) // MultiSignature::Sr25519
                .fixed(extra)
                .fixed(callBytes)
                .toArray();

        byte[] framed = new ScaleWriter().bytes(body).toArray(); // compact-length prefix
        String hex = "0x" + toHex(framed);
        return api.execute(RpcCall.create(Hash256.class, "author_submitExtrinsic", hex)).get();
    }

    @Override
    public void close() {
        api.close();
    }

    // ---- helpers ----

    private static byte[] concat(byte[]... parts) {
        int n = 0;
        for (byte[] p : parts) n += p.length;
        byte[] out = new byte[n];
        int o = 0;
        for (byte[] p : parts) { System.arraycopy(p, 0, out, o, p.length); o += p.length; }
        return out;
    }

    private static byte[] blake2b256(byte[] data) {
        Blake2bDigest d = new Blake2bDigest(256);
        d.update(data, 0, data.length);
        byte[] out = new byte[32];
        d.doFinal(out, 0);
        return out;
    }

    private static String toHex(byte[] b) {
        StringBuilder sb = new StringBuilder(b.length * 2);
        for (byte x : b) sb.append(String.format("%02x", x));
        return sb.toString();
    }
}
