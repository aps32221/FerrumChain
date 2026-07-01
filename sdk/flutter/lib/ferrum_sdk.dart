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
  static const String defaultEndpoint = 'ws://122.116.183.3:9944';

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
    final uri = Uri.parse(endpoint);
    if (uri.scheme == 'ws' || uri.scheme == 'wss') {
      // Connect explicitly (autoConnect fires an *unawaited* connect() whose
      // failures escape as uncaught async errors and whose stale channel races
      // with reconnects — the latter trips a `!` on a null query in polkadart).
      final ws = WsProvider(uri, autoConnect: false);
      await ws.connect();
      return FerrumClient._(ws);
    }
    return FerrumClient._(Provider.fromUri(uri));
  }

  /// Build a sr25519 keypair from a secret URI (e.g. "//Alice") or mnemonic.
  static Future<KeyPair> keypair(String uri) => KeyPair.sr25519.fromUri(uri);

  /// Send a JSON-RPC request and unwrap its result, surfacing node-side errors
  /// as a [FerrumRpcException]. Without this, a JSON-RPC error response (where
  /// `result` is null) would blow up as a confusing
  /// "type 'Null' is not a subtype of type 'String'" cast failure.
  Future<T> _rpc<T>(String method, List<dynamic> params) async {
    final resp = await provider.send(method, params);
    if (resp.error != null) {
      throw FerrumRpcException(method, resp.error);
    }
    final result = resp.result;
    if (result is! T) {
      throw FerrumRpcException(
          method, 'unexpected result type for $method: $result');
    }
    return result;
  }

  /// Sign and submit a Ferrum call; returns the extrinsic hash.
  Future<String> signAndSend(FerrumCall call, KeyPair signer) async {
    final pub = signer.publicKey.bytes;
    final callBytes = call.encode();

    final rv = await _rpc<Map<String, dynamic>>('state_getRuntimeVersion', []);
    final genesisHex = await _rpc<String>('chain_getBlockHash', [0]);
    final nonce = await _rpc<int>('system_accountNextIndex', [signer.address]);

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
    return _rpc<String>('author_submitExtrinsic', [hex]);
  }

  /// Free balance of [accountId] (a 32-byte sr25519 public key), in the chain's
  /// smallest unit. Reads `System.Account` storage directly and SCALE-decodes
  /// the `free` field; returns zero if the account has never been touched.
  Future<BigInt> balanceOf(Uint8List accountId) async {
    final key = _systemAccountKey(accountId);
    final hex =
        await _rpc<String?>('state_getStorage', ['0x${hexEncode(key)}']);
    if (hex == null) return BigInt.zero;
    final data = hexDecode(hex);
    // AccountInfo { nonce u32, consumers u32, providers u32, sufficients u32,
    // data: AccountData { free u128, reserved u128, frozen u128, flags u128 } }.
    // `free` therefore begins at byte 16.
    if (data.length < 32) {
      throw FerrumRpcException(
          'state_getStorage', 'short AccountInfo (${data.length} bytes)');
    }
    return _u128le(data, 16);
  }

  /// The chain's token decimals and symbol (from `system_properties`), used to
  /// render balances. Falls back to 12 decimals / "FER" if unspecified.
  Future<({int decimals, String symbol})> chainProperties() async {
    final props = await _rpc<Map<String, dynamic>>('system_properties', []);
    return (
      decimals: _firstInt(props['tokenDecimals']) ?? 12,
      symbol: _firstString(props['tokenSymbol']) ?? 'FER',
    );
  }

  static Uint8List _systemAccountKey(Uint8List accountId) {
    // twox128("System") ++ twox128("Account") ++ blake2_128_concat(accountId).
    final prefix = Hasher.twoxx128.hashString('System');
    final method = Hasher.twoxx128.hashString('Account');
    final hashed = Hasher.blake2b128.hash(accountId);
    return _concat([prefix, method, hashed, accountId]);
  }

  static BigInt _u128le(Uint8List b, int offset) {
    var v = BigInt.zero;
    for (var i = 15; i >= 0; i--) {
      v = (v << 8) | BigInt.from(b[offset + i]);
    }
    return v;
  }

  static int? _firstInt(dynamic v) {
    if (v is int) return v;
    if (v is List && v.isNotEmpty && v.first is int) return v.first as int;
    return null;
  }

  static String? _firstString(dynamic v) {
    if (v is String) return v;
    if (v is List && v.isNotEmpty && v.first is String)
      return v.first as String;
    return null;
  }

  Future<void> disconnect() async {
    // polkadart throws if the channel is already closed; treat that as success
    // so reconnects never fail on a stale/closed provider.
    try {
      await provider.disconnect();
    } catch (_) {/* already disconnected */}
  }

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
    final d = Blake2bDigest(digestSize: 32);
    d.update(data, 0, data.length);
    final out = Uint8List(32);
    d.doFinal(out, 0);
    return out;
  }
}

/// Thrown when a Ferrum node returns a JSON-RPC error (or an unexpected result
/// shape) for a request. Carries the node's error so the real reason — e.g. an
/// invalid/rejected extrinsic — is visible instead of an opaque cast failure.
class FerrumRpcException implements Exception {
  final String method;
  final Object? error;
  FerrumRpcException(this.method, this.error);

  @override
  String toString() {
    final e = error;
    if (e is Map) {
      final message = e['message'] ?? e;
      final data = e['data'];
      return 'Ferrum RPC "$method" failed: $message'
          '${data != null ? ' ($data)' : ''}';
    }
    return 'Ferrum RPC "$method" failed: $e';
  }
}
