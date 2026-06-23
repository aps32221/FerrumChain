# ferrum-sdk — Java

Typed thin wrapper over [`polkaj`](https://github.com/emeraldpay/polkaj) for the
Ferrum sovereign blockchain. polkaj provides the WebSocket transport, sr25519
(schnorrkel) signing and SS58 codec; the SDK SCALE-encodes Ferrum call parameters
itself (verified against the runtime's pallet/call indices) and assembles a v4
signed extrinsic with the runtime's 8-field `SignedExtra`.

## Install (Maven)

```xml
<dependency>
  <groupId>network.ferrum</groupId>
  <artifactId>ferrum-sdk</artifactId>
  <version>0.1.0</version>
</dependency>
```

## Quickstart

```java
import java.math.BigInteger;
import java.util.List;
import io.emeraldpay.polkaj.schnorrkel.Schnorrkel;
import network.ferrum.sdk.*;
import network.ferrum.sdk.FerrumTypes.*;

try (FerrumClient ferrum = FerrumClient.connect("ws://127.0.0.1:9944")) {
    Schnorrkel.KeyPair issuer = FerrumClient.keypairFromSeed(seed32);
    Did subject = Did.of("tw", "citizen-0001");

    Calls.FerrumCall anchor = ferrum.identity.anchorDid(new DidDocument(
        subject, issuer.getPublicKey(),
        "0x…",                                   // a commitment computed off-chain — no PII
        List.of(new DidKeyRef(KeyKind.Sr25519, "0x…")),
        "0x…", 0));
    System.out.println(ferrum.signAndSend(anchor, issuer));
}
```

Run the example against `ferrum-node --dev`:

```bash
mvn -q compile exec:java -Dexec.mainClass=network.ferrum.sdk.Example
```

## API shape

`FerrumClient` exposes one namespace per pallet; each method returns a
`Calls.FerrumCall` submitted with `signAndSend`:

```
ferrum.identity     anchorDid · rotateKeys · updateRevocation · registerIssuer
ferrum.credential   issue · revoke · setStatus · logPresentation
ferrum.tax          anchorInvoice · withhold · fileObligation · proveBracket · settle · authorizeAudit · setBrackets
ferrum.treasury     mint · burn · subsidize · recordSettlement
ferrum.federation   propose · vote · close · setMembership · setBasket · mintXsu · redeemXsu · bookClearing · netAndSettle · publishProofOfReserve
ferrum.interop      registerIssuer · submitInstruction · verifyFinality · netAndSettle · registerValidator · slashValidator
                    initAuthoritySet · rotateAuthoritySet · registerIssuerVk · verifyForeignProof · registerTreaty
                    recognizeForeignInvoice · ossRegister · ossReport
```

### Conventions

- 32-byte fields are `…Hex` strings (`0x…`); arbitrary blobs are `byte[]`.
- Tags/country/currency are short ASCII strings (`"tw"`, `"TW"`, `"TWD"`),
  length-checked at encode time.
- Variant fields use the Java enums in `FerrumTypes` whose ordinals match the
  on-chain SCALE discriminants.
- Rates are `double` fractions of one (`0.05` = 5%); amounts are `BigInteger`.

The signed extrinsic uses an **immortal** era and zero tip; the `SignedExtra`
ordering matches `runtime/src/lib.rs` exactly. Personal-data fields only accept
commitments/hashes — you cannot put plaintext PII into an extrinsic
(whitepaper §03/§05/§06/§09).
