import 'dart:typed_data';

/// Minimal SCALE encoder for the Ferrum call types. polkadart supplies the WS
/// transport, sr25519 signing and SS58 codec; this writer produces the call and
/// signing-payload bytes so the SDK stays a thin, correct wrapper.
class ScaleWriter {
  final BytesBuilder _buf = BytesBuilder();

  Uint8List toBytes() => _buf.toBytes();

  ScaleWriter u8(int v) {
    _buf.addByte(v & 0xFF);
    return this;
  }

  ScaleWriter boolean(bool v) {
    _buf.addByte(v ? 1 : 0);
    return this;
  }

  ScaleWriter u32(int v) {
    for (var i = 0; i < 4; i++) {
      _buf.addByte((v >> (8 * i)) & 0xFF);
    }
    return this;
  }

  ScaleWriter u64(int v) {
    var x = BigInt.from(v);
    for (var i = 0; i < 8; i++) {
      _buf.addByte((x >> (8 * i)).toUnsigned(8).toInt() & 0xFF);
    }
    return this;
  }

  ScaleWriter u128(BigInt v) {
    if (v.isNegative) throw ArgumentError('u128 must be non-negative');
    for (var i = 0; i < 16; i++) {
      _buf.addByte((v >> (8 * i)).toUnsigned(8).toInt() & 0xFF);
    }
    return this;
  }

  /// Raw fixed-width bytes ([u8; N]) — no length prefix.
  ScaleWriter fixed(Uint8List v) {
    _buf.add(v);
    return this;
  }

  /// SCALE compact-encoded unsigned integer.
  ScaleWriter compact(BigInt v) {
    if (v.isNegative) throw ArgumentError('compact must be non-negative');
    if (v < BigInt.from(64)) {
      _buf.addByte((v.toInt() << 2));
    } else if (v < BigInt.from(16384)) {
      final x = (v.toInt() << 2) | 1;
      _buf.addByte(x & 0xFF);
      _buf.addByte((x >> 8) & 0xFF);
    } else if (v < BigInt.from(1073741824)) {
      final x = (v.toInt() << 2) | 2;
      for (var i = 0; i < 4; i++) {
        _buf.addByte((x >> (8 * i)) & 0xFF);
      }
    } else {
      final le = <int>[];
      var t = v;
      while (t > BigInt.zero) {
        le.add((t & BigInt.from(0xFF)).toInt());
        t = t >> 8;
      }
      _buf.addByte(((le.length - 4) << 2) | 3);
      _buf.add(le);
    }
    return this;
  }

  /// Length-prefixed byte vector (Vec<u8> / BoundedVec<u8> / Bytes).
  ScaleWriter bytes(Uint8List v) {
    compact(BigInt.from(v.length));
    _buf.add(v);
    return this;
  }

  /// ASCII string as a length-prefixed byte vector (BoundedVec<u8>).
  ScaleWriter asciiVec(String s) => bytes(Uint8List.fromList(s.codeUnits));

  /// Fixed-width ASCII code ([u8;N]) — length-checked, no prefix.
  ScaleWriter asciiFixed(String s, int len) {
    final b = Uint8List.fromList(s.codeUnits);
    if (b.length != len) {
      throw ArgumentError("code '$s' must be exactly $len ASCII bytes");
    }
    return fixed(b);
  }

  /// Vec<T> with a custom element writer.
  ScaleWriter vec<T>(List<T> items, void Function(ScaleWriter, T) writeElem) {
    compact(BigInt.from(items.length));
    for (final it in items) {
      writeElem(this, it);
    }
    return this;
  }

  /// Option<T>: None=0x00, Some=0x01 then the value.
  ScaleWriter option<T>(T? value, void Function(ScaleWriter, T) writeSome) {
    if (value == null) return u8(0);
    u8(1);
    writeSome(this, value);
    return this;
  }
}

Uint8List hexDecode(String hex) {
  final s = hex.startsWith('0x') ? hex.substring(2) : hex;
  final out = Uint8List(s.length ~/ 2);
  for (var i = 0; i < out.length; i++) {
    out[i] = int.parse(s.substring(i * 2, i * 2 + 2), radix: 16);
  }
  return out;
}

Uint8List hex32(String hex) {
  final b = hexDecode(hex);
  if (b.length != 32) throw ArgumentError('expected 32 bytes, got ${b.length}');
  return b;
}

String hexEncode(Uint8List b) =>
    b.map((x) => x.toRadixString(16).padLeft(2, '0')).join();

/// Perbill parts-per-billion from a fraction of one (0..1).
int perbill(double frac) {
  if (frac < 0 || frac > 1) {
    throw ArgumentError('perbill fraction must be within [0,1]');
  }
  return (frac * 1000000000).round();
}
