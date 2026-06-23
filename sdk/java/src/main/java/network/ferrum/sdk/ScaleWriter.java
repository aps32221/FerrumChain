package network.ferrum.sdk;

import java.io.ByteArrayOutputStream;
import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.util.List;
import java.util.function.BiConsumer;

/**
 * Minimal SCALE encoder for the Ferrum call types. polkaj supplies the WS
 * transport, sr25519 signing and SS58 codec; this writer produces the call and
 * signing-payload bytes so the SDK stays a thin, correct wrapper.
 */
public final class ScaleWriter {
    private final ByteArrayOutputStream buf = new ByteArrayOutputStream();

    public byte[] toArray() { return buf.toByteArray(); }

    public ScaleWriter u8(int v) { buf.write(v & 0xFF); return this; }
    public ScaleWriter bool(boolean v) { buf.write(v ? 1 : 0); return this; }

    public ScaleWriter u32(long v) {
        for (int i = 0; i < 4; i++) buf.write((int) ((v >> (8 * i)) & 0xFF));
        return this;
    }

    public ScaleWriter u64(long v) {
        for (int i = 0; i < 8; i++) buf.write((int) ((v >> (8 * i)) & 0xFF));
        return this;
    }

    public ScaleWriter u128(BigInteger v) {
        if (v.signum() < 0) throw new IllegalArgumentException("u128 must be non-negative");
        byte[] le = toLittleEndian(v, 16);
        buf.writeBytes(le);
        return this;
    }

    /** Raw fixed-width bytes ([u8; N]) — no length prefix. */
    public ScaleWriter fixed(byte[] v) { buf.writeBytes(v); return this; }

    /** SCALE compact-encoded unsigned integer. */
    public ScaleWriter compact(BigInteger v) {
        if (v.signum() < 0) throw new IllegalArgumentException("compact must be non-negative");
        BigInteger u64Max = BigInteger.ONE.shiftLeft(30);
        if (v.compareTo(BigInteger.valueOf(64)) < 0) {
            buf.write(v.intValue() << 2);
        } else if (v.compareTo(BigInteger.valueOf(16384)) < 0) {
            int x = (v.intValue() << 2) | 1;
            buf.write(x & 0xFF); buf.write((x >> 8) & 0xFF);
        } else if (v.compareTo(u64Max) < 0) {
            long x = (v.longValue() << 2) | 2;
            for (int i = 0; i < 4; i++) buf.write((int) ((x >> (8 * i)) & 0xFF));
        } else {
            byte[] le = toLittleEndian(v, 0);
            buf.write(((le.length - 4) << 2) | 3);
            buf.writeBytes(le);
        }
        return this;
    }

    public ScaleWriter compact(long v) { return compact(BigInteger.valueOf(v)); }

    /** Length-prefixed byte vector (Vec<u8> / BoundedVec<u8> / Bytes). */
    public ScaleWriter bytes(byte[] v) { compact(BigInteger.valueOf(v.length)); buf.writeBytes(v); return this; }

    /** ASCII string as a length-prefixed byte vector (BoundedVec<u8>). */
    public ScaleWriter asciiVec(String s) { return bytes(s.getBytes(StandardCharsets.US_ASCII)); }

    /** Fixed-width ASCII code ([u8;N]) — length-checked, no prefix. */
    public ScaleWriter asciiFixed(String s, int len) {
        byte[] b = s.getBytes(StandardCharsets.US_ASCII);
        if (b.length != len) throw new IllegalArgumentException("code '" + s + "' must be exactly " + len + " ASCII bytes");
        return fixed(b);
    }

    /** Vec<T> with a custom element writer. */
    public <T> ScaleWriter vec(List<T> items, BiConsumer<ScaleWriter, T> writeElem) {
        compact(BigInteger.valueOf(items.size()));
        for (T it : items) writeElem.accept(this, it);
        return this;
    }

    /** Option<T>: None=0x00, Some=0x01 then the value. */
    public <T> ScaleWriter option(T value, BiConsumer<ScaleWriter, T> writeSome) {
        if (value == null) return u8(0);
        u8(1);
        writeSome.accept(this, value);
        return this;
    }

    private static byte[] toLittleEndian(BigInteger v, int padTo) {
        byte[] be = v.toByteArray();
        int start = (be.length > 1 && be[0] == 0) ? 1 : 0; // strip sign byte
        int len = be.length - start;
        int outLen = Math.max(len, padTo);
        byte[] le = new byte[outLen];
        for (int i = 0; i < len; i++) le[i] = be[be.length - 1 - i];
        return le;
    }

    // ---- shared helpers ----

    public static byte[] hex(String h) {
        String s = h.startsWith("0x") ? h.substring(2) : h;
        int n = s.length() / 2;
        byte[] out = new byte[n];
        for (int i = 0; i < n; i++) out[i] = (byte) Integer.parseInt(s.substring(i * 2, i * 2 + 2), 16);
        return out;
    }

    public static byte[] hex32(String h) {
        byte[] b = hex(h);
        if (b.length != 32) throw new IllegalArgumentException("expected 32 bytes, got " + b.length);
        return b;
    }

    public static long perbill(double frac) {
        if (frac < 0 || frac > 1) throw new IllegalArgumentException("perbill fraction must be within [0,1]");
        return Math.round(frac * 1_000_000_000.0);
    }
}
