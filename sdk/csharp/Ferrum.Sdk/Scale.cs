using System.Numerics;
using System.Text;

namespace Ferrum.Sdk;

/// <summary>
/// Minimal SCALE encoder for the Ferrum call types. Substrate.NetApi handles
/// transport and signing; these helpers produce the call-parameter bytes so the
/// SDK stays a thin, correct wrapper that cannot drift from a stale codegen.
/// </summary>
public sealed class ScaleWriter
{
    private readonly List<byte> _buf = new();

    public byte[] ToArray() => _buf.ToArray();

    public ScaleWriter U8(byte v) { _buf.Add(v); return this; }
    public ScaleWriter Bool(bool v) { _buf.Add((byte)(v ? 1 : 0)); return this; }

    public ScaleWriter U32(uint v) { _buf.AddRange(BitConverter.GetBytes(v)); return this; } // LE
    public ScaleWriter U64(ulong v) { _buf.AddRange(BitConverter.GetBytes(v)); return this; }

    public ScaleWriter U128(BigInteger v)
    {
        if (v.Sign < 0) throw new ArgumentException("u128 must be non-negative");
        var bytes = v.ToByteArray(isUnsigned: true, isBigEndian: false);
        if (bytes.Length > 16) throw new ArgumentException("value exceeds u128");
        _buf.AddRange(bytes);
        _buf.AddRange(new byte[16 - bytes.Length]); // pad to 16 LE
        return this;
    }

    /// <summary>Raw fixed-width bytes ([u8; N]) — no length prefix.</summary>
    public ScaleWriter Fixed(byte[] v) { _buf.AddRange(v); return this; }

    /// <summary>SCALE compact-encoded unsigned integer.</summary>
    public ScaleWriter Compact(BigInteger v)
    {
        if (v < 0) throw new ArgumentException("compact must be non-negative");
        if (v < 64) { _buf.Add((byte)(v << 2)); return this; }
        if (v < 16384) { uint x = (uint)((v << 2) | 1); _buf.AddRange(BitConverter.GetBytes((ushort)x)); return this; }
        if (v < 1073741824) { uint x = (uint)((v << 2) | 2); _buf.AddRange(BitConverter.GetBytes(x)); return this; }
        var bytes = v.ToByteArray(isUnsigned: true, isBigEndian: false);
        _buf.Add((byte)(((bytes.Length - 4) << 2) | 3));
        _buf.AddRange(bytes);
        return this;
    }

    /// <summary>Length-prefixed byte vector (Vec&lt;u8&gt; / BoundedVec&lt;u8&gt; / Bytes).</summary>
    public ScaleWriter Bytes(byte[] v) { Compact(v.Length); _buf.AddRange(v); return this; }

    /// <summary>ASCII string as a length-prefixed byte vector (BoundedVec&lt;u8&gt;).</summary>
    public ScaleWriter AsciiVec(string s) => Bytes(Encoding.ASCII.GetBytes(s));

    /// <summary>Fixed-width ASCII code ([u8;N]) — length-checked, no prefix.</summary>
    public ScaleWriter AsciiFixed(string s, int len)
    {
        var b = Encoding.ASCII.GetBytes(s);
        if (b.Length != len) throw new ArgumentException($"code '{s}' must be exactly {len} ASCII bytes");
        return Fixed(b);
    }

    /// <summary>Enum/variant discriminant byte followed by any inline fields.</summary>
    public ScaleWriter Variant(byte index) => U8(index);

    /// <summary>Vec&lt;T&gt; with a custom element writer.</summary>
    public ScaleWriter Vec<T>(IReadOnlyList<T> items, Action<ScaleWriter, T> writeElem)
    {
        Compact(items.Count);
        foreach (var it in items) writeElem(this, it);
        return this;
    }

    /// <summary>Option&lt;T&gt;: None=0x00, Some=0x01 then the value.</summary>
    public ScaleWriter Option<T>(T? value, Action<ScaleWriter, T> writeSome) where T : struct
    {
        if (value is null) return U8(0);
        U8(1);
        writeSome(this, value.Value);
        return this;
    }
}

public static class Hex
{
    public static byte[] Decode(string hex)
    {
        var s = hex.StartsWith("0x") ? hex[2..] : hex;
        if (s.Length % 2 != 0) throw new ArgumentException("odd-length hex");
        var bytes = new byte[s.Length / 2];
        for (int i = 0; i < bytes.Length; i++) bytes[i] = Convert.ToByte(s.Substring(i * 2, 2), 16);
        return bytes;
    }

    public static byte[] Bytes32(string hex)
    {
        var b = Decode(hex);
        if (b.Length != 32) throw new ArgumentException($"expected 32 bytes, got {b.Length}");
        return b;
    }

    /// <summary>Perbill parts-per-billion from a fraction of one (0..1).</summary>
    public static uint Perbill(double frac)
    {
        if (frac is < 0 or > 1) throw new ArgumentException("perbill fraction must be within [0,1]");
        return (uint)Math.Round(frac * 1_000_000_000.0);
    }
}
