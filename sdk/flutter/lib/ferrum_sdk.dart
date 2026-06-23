/// Typed thin wrapper over [polkadart](https://github.com/leonardocustodio/polkadart)
/// for the Ferrum sovereign blockchain.
library ferrum_sdk;

import 'dart:typed_data';

import 'package:pointycastle/digests/blake2b.dart';
import 'package:polkadart/polkadart.dart';
import 'package:polkadart_keyring/polkadart_keyring.dart';

import 'src/calls.dart';
import 'src/scale_writer.dart';

export 'src/calls.dart';
export 'src/scale_writer.dart' show hexDecode, hex32, hexEncode, perbill;
export 'src/types.dart';

/// A connected Ferrum client.
///
/// polkadart provides the WS transport, sr25519 signing and SS58 codec; this
/// client SCALE-encodes the call params and assembles a v4 signed extrinsic with
/// the runtime's 8-field SignedExtra (immortal era, zero tip).
class FerrumClient {
  static const String defaultEndpoint = 'ws://127.0.0.1:9944';

  final Provider provider;

  final IdentityCalls identity = IdentityCalls();
  final CredentialCalls credential = CredentialCalls();
  final TaxCalls tax = TaxCalls();
  final TreasuryCalls treasury = TreasuryCalls();
  final FederationCalls federation = FederationCalls();
  final InteropCalls interop = InteropCalls();

  FerrumClient._(this.provider);

  static Future<FerrumClient> connect(
      [String endpoint = defaultEndpoint]) async {
    final provider = Provider.fromUri(Uri.parse(endpoint));
    return FerrumClient._(provider);
  }

  /// Build a sr25519 keypair from a secret URI (e.g. "//Alice") or mnemonic.
  static Future<KeyPair> keypair(String uri) => KeyPair.sr25519.fromUri(uri);

  /// Sign and submit a Ferrum call; returns the extrinsic hash.
  Future<String> signAndSend(FerrumCall call, KeyPair signer) async {
    final pub = signer.publicKey.bytes;
    final callBytes = call.encode();

    final rv = (await provider.send('state_getRuntimeVersion', [])).result
        as Map<String, dynamic>;
    final genesisHex =
        (await provider.send('chain_getBlockHash', [0])).result as String;
    final nonce =
        (await provider.send('system_accountNextIndex', [signer.address]))
            .result as int;

    final genesis = hex32(genesisHex);
    final extra = (ScaleWriter()
          ..u8(0) // immortal era
          ..compact(BigInt.from(nonce))
          ..compact(BigInt.zero)) // tip
        .toBytes();
    final additional = (ScaleWriter()
          ..u32(rv['specVersion'] as int)
          ..u32(rv['transactionVersion'] as int)
          ..fixed(genesis)
          ..fixed(genesis)) // immortal -> block hash = genesis
        .toBytes();

    var payload = _concat([callBytes, extra, additional]);
    if (payload.length > 256) payload = _blake2b256(payload);
    final sig = signer.sign(payload);

    final body = (ScaleWriter()
          ..u8(0x84) // v4, signed
          ..u8(0x00)
          ..fixed(Uint8List.fromList(pub)) // MultiAddress::Id
          ..u8(0x01)
          ..fixed(Uint8List.fromList(sig)) // MultiSignature::Sr25519
          ..fixed(extra)
          ..fixed(callBytes))
        .toBytes();

    final framed = (ScaleWriter()..bytes(body)).toBytes();
    final hex = '0x${hexEncode(framed)}';
    return (await provider.send('author_submitExtrinsic', [hex])).result
        as String;
  }

  Future<void> disconnect() => provider.disconnect();

  static Uint8List _concat(List<Uint8List> parts) {
    final n = parts.fold<int>(0, (a, p) => a + p.length);
    final out = Uint8List(n);
    var o = 0;
    for (final p in parts) {
      out.setRange(o, o + p.length, p);
      o += p.length;
    }
    return out;
  }

  static Uint8List _blake2b256(Uint8List data) {
    final d = Blake2bDigest(null, 32);
    d.update(data, 0, data.length);
    final out = Uint8List(32);
    d.doFinal(out, 0);
    return out;
  }
}
