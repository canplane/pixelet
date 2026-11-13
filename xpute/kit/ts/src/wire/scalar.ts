// @xpute/kit/wire/scalar.ts

/**
 * Binary Scalar I/O
 *
 * - All operations are little-endian by specification.
 * - No validation. Caller owns correctness.
 *
 * RULE
 * ----
 * This is IO only: raw byte stream read/write.
 * Signedness is a *meaning* decision; we expose both geti/getu for native widths,
 * but we do not "fix" or "reinterpret" beyond what DataView already does.
 *
 * Composite widths (96/128) are limb-IO helpers.
 * If you need signed semantics for sub-fields, use abi/cast.ts.
 */

import type {
  f32,
  f64,
  i16,
  i32,
  i64,
  i8,
  u16,
  u32,
  u64,
  u8,
} from "@xpute/kit/abi/word.ts";
import { U64 } from "@xpute/kit/abi/word.ts";
import type { ByteArray } from "@xpute/kit/abi/array.ts";

// ----------------------------------------------------------------------------
// Stream IO (little-endian)
// - No validation: caller owns bounds checks.
// ----------------------------------------------------------------------------

export interface Stream {
  view: DataView;
  buf: ByteArray;
  off: i32;
}

export const stream = (buf: ByteArray, off: i32 = 0): Stream => ({
  buf,
  view: new DataView(buf.buffer, buf.byteOffset, buf.byteLength),
  off,
});

// ----------------------------------------------------------------------------
// memcpy (raw memory copy)
// - No validation: caller owns correctness (RangeError is fine)
// ----------------------------------------------------------------------------

export const memcpy = (dst: ByteArray, src: ByteArray, nbyte: u32): void => {
  // force exact-length source view; dst range is enforced by dst view length via .set()
  const s = new Uint8Array(src.buffer, src.byteOffset, nbyte);
  dst.set(s, 0); // RangeError if dst.byteLength < nbyte
};

export const write = (s: Stream, src: ByteArray, nbyte: u32): void => {
  const beg = s.off;
  const end = beg + nbyte;
  // exact-length view: RangeError if src's buffer ends before byteOffset + nbyte
  const src_n = new Uint8Array(src.buffer, src.byteOffset, nbyte);
  s.buf.set(src_n, beg);
  s.off = end;
};
export const read = (s: Stream, dst: ByteArray, nbyte: u32): void => {
  const beg = s.off;
  const end = beg + nbyte;
  // exact-length view: RangeError if the stream's buffer ends before the read does
  const src_n = new Uint8Array(s.buf.buffer, s.buf.byteOffset + beg, nbyte);
  dst.set(src_n, 0);
  s.off = end;
};

/** `seek` counts from these, as lseek(2)'s whence does. */
export const SEEK_SET = 0;
export const SEEK_CUR = 1;
export const SEEK_END = 2;

export const seek = (s: Stream, off: i32, whence: u8 = SEEK_SET): i32 => {
  if (whence === SEEK_CUR) s.off = s.off + off;
  else if (whence === SEEK_END) s.off = s.view.byteLength + off;
  else s.off = off;
  return s.off;
};

export const tell = (s: Stream): i32 => s.off;
export const len = (s: Stream): i32 => s.view.byteLength;
export const rem = (s: Stream): i32 => s.view.byteLength - s.off;

// ----------------------------------------------------------------------------
// scalar ops (advance stream)
// ----------------------------------------------------------------------------

export const putu8 = (s: Stream, x: u8): void =>
  s.view.setUint8((s.off += 1) - 1, x);
export const puti8 = (s: Stream, x: i8): void =>
  s.view.setInt8((s.off += 1) - 1, x);
export const getu8 = (s: Stream): u8 => s.view.getUint8((s.off += 1) - 1);
export const geti8 = (s: Stream): i8 => s.view.getInt8((s.off += 1) - 1);

export const putu16 = (s: Stream, x: u16): void =>
  s.view.setUint16((s.off += 2) - 2, x, true);
export const puti16 = (s: Stream, x: i16): void =>
  s.view.setInt16((s.off += 2) - 2, x, true);
export const getu16 = (s: Stream): u16 =>
  s.view.getUint16((s.off += 2) - 2, true);
export const geti16 = (s: Stream): i16 =>
  s.view.getInt16((s.off += 2) - 2, true);

export const putu32 = (s: Stream, x: u32): void =>
  s.view.setUint32((s.off += 4) - 4, x, true);
export const puti32 = (s: Stream, x: i32): void =>
  s.view.setInt32((s.off += 4) - 4, x, true);
export const getu32 = (s: Stream): u32 =>
  s.view.getUint32((s.off += 4) - 4, true);
export const geti32 = (s: Stream): i32 =>
  s.view.getInt32((s.off += 4) - 4, true);

export const putu64 = (s: Stream, x: u64): void =>
  s.view.setBigUint64((s.off += 8) - 8, x, true);
export const puti64 = (s: Stream, x: i64): void =>
  s.view.setBigInt64((s.off += 8) - 8, x, true);
export const getu64 = (s: Stream): u64 =>
  s.view.getBigUint64((s.off += 8) - 8, true);
export const geti64 = (s: Stream): i64 =>
  s.view.getBigInt64((s.off += 8) - 8, true);

export const putf32 = (s: Stream, x: f32): void =>
  s.view.setFloat32((s.off += 4) - 4, x, true);
export const getf32 = (s: Stream): f32 =>
  s.view.getFloat32((s.off += 4) - 4, true);

export const putf64 = (s: Stream, x: f64): void =>
  s.view.setFloat64((s.off += 8) - 8, x, true);
export const getf64 = (s: Stream): f64 =>
  s.view.getFloat64((s.off += 8) - 8, true);

// ----------------------------------------------------------------------------
// 96/128 as limb stream IO (little-endian)
// ----------------------------------------------------------------------------

export const U96_SZ: u32 = 12;
export const U128_SZ: u32 = 16;

export const putu96 = (s: Stream, lo: u32, mid: u32, hi: u32): void => {
  putu32(s, lo);
  putu32(s, mid);
  putu32(s, hi);
};

export const getu96 = (s: Stream): [u32, u32, u32] => {
  const lo = getu32(s);
  const mid = getu32(s);
  const hi = getu32(s);
  return [lo, mid, hi];
};

export const putu128 = (s: Stream, lo: u64, hi: u64): void => {
  putu64(s, lo);
  putu64(s, hi);
};

export const getu128 = (s: Stream): [u64, u64] => {
  const lo = getu64(s);
  const hi = getu64(s);
  return [lo, hi];
};

// ----------------------------------------------------------------------------
// 96/128 bigint <-> limb (keep)
// ----------------------------------------------------------------------------

export const pack96 = (lo: u32, mid: u32, hi: u32): bigint =>
  (U64(BigInt(hi)) << 64n) | (U64(BigInt(mid)) << 32n) | U64(BigInt(lo));

// Each 32-bit part is masked while it is still a bigint. Number() of a value
// past 2^53 rounds away its low bits, which a later truncation to 32 bits
// would need.
export const unpack96 = (x: bigint): [u32, u32, u32] => [
  Number(x & 0xffffffffn),
  Number((x >> 32n) & 0xffffffffn),
  Number((x >> 64n) & 0xffffffffn),
];

export const pack128 = (lo: u64, hi: u64): bigint => (U64(hi) << 64n) | U64(lo);

export const unpack128 = (x: bigint): [u64, u64] => [U64(x), U64(x >> 64n)];
