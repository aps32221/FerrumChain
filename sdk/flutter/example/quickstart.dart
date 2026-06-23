// Quickstart against a local dev node:  ./target/release/ferrum-node --dev
//
//   dart run example/quickstart.dart
//
// Anchors a DID and files a tax obligation as //Alice (the dev sudo/governance
// key — also accredit Alice as an issuer once via a sudo-wrapped registerIssuer).

import 'dart:typed_data';

import 'package:ferrum_sdk/ferrum_sdk.dart';
import 'package:pointycastle/digests/blake2b.dart';

String commit(String s) {
  final d = Blake2bDigest(null, 32);
  final bytes = Uint8List.fromList(s.codeUnits);
  d.update(bytes, 0, bytes.length);
  final out = Uint8List(32);
  d.doFinal(out, 0);
  return '0x${hexEncode(out)}';
}

Future<void> main() async {
  final ferrum = await FerrumClient.connect('ws://127.0.0.1:9944');
  final alice = await FerrumClient.keypair('//Alice');

  final subject = Did.of('tw', 'citizen-0001');

  final anchor = ferrum.identity.anchorDid(DidDocument(
    subject,
    Uint8List.fromList(alice.publicKey.bytes),
    commit('off-chain DID document for citizen #1'), // commitment only — no PII
    [DidKeyRef(KeyKind.sr25519, commit('device-key'))],
    commit('rev-acc-0'),
    0,
  ));
  print('anchor_did hash: ${await ferrum.signAndSend(anchor, alice)}');

  final obligation = ferrum.tax.fileObligation(TaxObligation(
    subject,
    TaxKind.income,
    FiatAmount('TWD', BigInt.from(1234500)),
    commit('encrypted return detail'),
    false,
  ));
  print('file_obligation hash: ${await ferrum.signAndSend(obligation, alice)}');

  await ferrum.disconnect();
}
