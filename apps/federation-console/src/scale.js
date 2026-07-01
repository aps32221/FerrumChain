// Minimal SCALE encoder — a byte-for-byte port of the C# SDK's ScaleWriter
// (sdk/csharp/Ferrum.Sdk/Scale.cs). Produces the same call-parameter bytes so
// extrinsics built here are identical to the reference SDK.
import { hexToU8a, stringToU8a, u8aConcat } from '@polkadot/util'

export class ScaleWriter {
  constructor() {
    this.parts = []
  }
  _push(u8a) {
    this.parts.push(u8a)
    return this
  }
  toU8a() {
    return u8aConcat(...this.parts)
  }

  u8(v) {
    return this._push(Uint8Array.of(v & 0xff))
  }
  bool(v) {
    return this.u8(v ? 1 : 0)
  }
  // little-endian fixed-width integers
  u32(v) {
    return this._push(leBytes(BigInt(v), 4))
  }
  u64(v) {
    return this._push(leBytes(BigInt(v), 8))
  }
  u128(v) {
    const b = BigInt(v)
    if (b < 0n) throw new Error('u128 must be non-negative')
    if (b >= 1n << 128n) throw new Error('value exceeds u128')
    return this._push(leBytes(b, 16))
  }
  // raw fixed-width bytes ([u8; N]) — no length prefix
  fixed(u8a) {
    return this._push(asU8a(u8a))
  }
  // SCALE compact-encoded unsigned integer
  compact(v) {
    let n = BigInt(v)
    if (n < 0n) throw new Error('compact must be non-negative')
    if (n < 64n) return this.u8(Number(n << 2n))
    if (n < 16384n) return this._push(leBytes((n << 2n) | 1n, 2))
    if (n < 1073741824n) return this._push(leBytes((n << 2n) | 2n, 4))
    const bytes = leTrim(n)
    this.u8(((bytes.length - 4) << 2) | 3)
    return this._push(bytes)
  }
  // length-prefixed byte vector (Vec<u8> / BoundedVec<u8> / Bytes)
  bytes(u8a) {
    const b = asU8a(u8a)
    this.compact(b.length)
    return this._push(b)
  }
  // ASCII string as a length-prefixed byte vector
  asciiVec(s) {
    return this.bytes(stringToU8a(s))
  }
  // fixed-width ASCII code ([u8; N]) — length-checked, no prefix
  asciiFixed(s, len) {
    const b = stringToU8a(s)
    if (b.length !== len) throw new Error(`code '${s}' must be exactly ${len} ASCII bytes`)
    return this.fixed(b)
  }
  // Vec<T> with a custom element writer
  vec(items, writeElem) {
    this.compact(items.length)
    for (const it of items) writeElem(this, it)
    return this
  }
}

// ---- helpers -------------------------------------------------------------
function leBytes(value, len) {
  const out = new Uint8Array(len)
  let v = BigInt(value)
  for (let i = 0; i < len; i++) {
    out[i] = Number(v & 0xffn)
    v >>= 8n
  }
  return out
}

function leTrim(value) {
  let v = BigInt(value)
  const out = []
  if (v === 0n) return Uint8Array.of(0)
  while (v > 0n) {
    out.push(Number(v & 0xffn))
    v >>= 8n
  }
  return Uint8Array.from(out)
}

function asU8a(x) {
  if (x instanceof Uint8Array) return x
  if (typeof x === 'string') return hexToU8a(x.startsWith('0x') ? x : '0x' + x)
  if (Array.isArray(x)) return Uint8Array.from(x)
  throw new Error('expected bytes')
}

// 32-byte hash from hex, length-checked.
export function bytes32(hex) {
  const b = asU8a(hex)
  if (b.length !== 32) throw new Error(`expected 32 bytes, got ${b.length}`)
  return b
}

// Perbill (parts-per-billion) from a percentage (0..100).
export function perbillFromPercent(pct) {
  const p = Number(pct)
  if (p < 0 || p > 100) throw new Error('percent must be within [0,100]')
  return Math.round((p / 100) * 1_000_000_000)
}

export { asU8a }
