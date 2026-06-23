package network.ferrum.sdk;

import java.math.BigInteger;
import java.security.MessageDigest;
import java.util.List;

import io.emeraldpay.polkaj.schnorrkel.Schnorrkel;
import network.ferrum.sdk.FerrumTypes.*;

/**
 * Quickstart against a local dev node:  ./target/release/ferrum-node --dev
 *
 *   mvn -q compile exec:java -Dexec.mainClass=network.ferrum.sdk.Example
 *
 * Supply a 32-byte issuer seed via FERRUM_SEED (hex). The DID's doc_hash is a
 * commitment computed off-chain — never the document itself.
 */
public final class Example {
    public static void main(String[] args) throws Exception {
        try (FerrumClient ferrum = FerrumClient.connect("ws://127.0.0.1:9944")) {
            String seedHex = System.getenv().getOrDefault("FERRUM_SEED", "00".repeat(32));
            Schnorrkel.KeyPair issuer = FerrumClient.keypairFromSeed(ScaleWriter.hex(seedHex));
            byte[] pub = issuer.getPublicKey();

            Did subject = Did.of("tw", "citizen-0001");

            Calls.FerrumCall anchor = ferrum.identity.anchorDid(new DidDocument(
                    subject, pub,
                    commit("off-chain DID document for citizen #1"),
                    List.of(new DidKeyRef(KeyKind.Sr25519, commit("device-key"))),
                    commit("rev-acc-0"), 0));
            System.out.println("anchor_did hash: " + ferrum.signAndSend(anchor, issuer));

            Calls.FerrumCall obligation = ferrum.tax.fileObligation(new TaxObligation(
                    subject, TaxKind.Income,
                    new FiatAmount("TWD", BigInteger.valueOf(1234500)),
                    commit("encrypted return detail"), false));
            System.out.println("file_obligation hash: " + ferrum.signAndSend(obligation, issuer));
        }
    }

    // Placeholder commitment; use BLAKE2b-256 to match the chain hashing in production.
    private static String commit(String s) throws Exception {
        byte[] h = MessageDigest.getInstance("SHA-256").digest(s.getBytes("UTF-8"));
        StringBuilder sb = new StringBuilder("0x");
        for (byte b : h) sb.append(String.format("%02x", b));
        return sb.toString();
    }
}
