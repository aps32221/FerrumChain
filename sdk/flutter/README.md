# ferrum_sdk — Flutter / Dart

Typed thin wrapper over [`polkadart`](https://github.com/leonardocustodio/polkadart)
for the Ferrum sovereign blockchain. polkadart provides the WebSocket transport,
sr25519 signing and SS58 codec; the SDK SCALE-encodes Ferrum call parameters itself
(verified against the runtime's pallet/call indices) and assembles a v4 signed
extrinsic with the runtime's 8-field `SignedExtra`.

## Install

```yaml
dependencies:
  ferrum_sdk:
    path: ../sdk/flutter   # or a published version
```

## Quickstart

```dart
import 'dart:typed_data';
import 'package:ferrum_sdk/ferrum_sdk.dart';

final ferrum = await FerrumClient.connect('ws://127.0.0.1:9944');
final alice = await FerrumClient.keypair('//Alice');

final subject = Did.of('tw', 'citizen-0001');
final anchor = ferrum.identity.anchorDid(DidDocument(
  subject,
  Uint8List.fromList(alice.publicKey.bytes),
  '0x…',                                    // a commitment computed off-chain — no PII
  [DidKeyRef(KeyKind.sr25519, '0x…')],
  '0x…',
  0,
));
print(await ferrum.signAndSend(anchor, alice));
await ferrum.disconnect();
```

Run the example against `ferrum-node --dev`:

```bash
dart pub get
dart run example/quickstart.dart
```

## API shape

`FerrumClient` exposes one namespace per pallet; each method returns a `FerrumCall`
submitted with `signAndSend`:

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

- 32-byte fields are `…Hex` strings (`0x…`); arbitrary blobs are `Uint8List`.
- Tags/country/currency are short ASCII strings (`'tw'`, `'TW'`, `'TWD'`),
  length-checked at encode time.
- Variant fields use the Dart enums (`KeyKind`, `TaxKind`, `Vote`, …) whose
  indices match the on-chain SCALE discriminants.
- Rates are `double` fractions of one (`0.05` = 5%); amounts are `BigInt`.

The signed extrinsic uses an **immortal** era and zero tip. Personal-data fields
only accept commitments/hashes — you cannot put plaintext PII into an extrinsic
(whitepaper §03/§05/§06/§09).
