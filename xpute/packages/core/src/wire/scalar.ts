// @xpute/core/wire/scalar.ts

// Little-endian scalar I/O on a byte stream, with no bounds checks: the caller
// owns them, and an out-of-range access throws DataView's RangeError.

import type { f32, f64, i16, i32, i64, i8, u16, u32, u64, u8 } from "@xpute/core/abi/word.ts";
import { U64 } from "@xpute/core/abi/word.ts";
import type { Bytes } from "@xpute/core/abi/array.ts";

export interface Stream {
  view: DataView;
  buf: Bytes;
  off: i32;
}

export const stream = (buf: Bytes, off: i32 = 0): Stream => ({
  buf,
  view: new DataView(buf.buffer, buf.byteOffset, buf.byteLength),
  off,
});

export const memcpy = (dst: Bytes, src: Bytes, nbyte: u32): void => {
  const s = new Uint8Array(src.buffer, src.byteOffset, nbyte);
  dst.set(s, 0);
};

export const write = (s: Stream, src: Bytes, nbyte: u32): void => {
  const beg = s.off;
  const end = beg + nbyte;
  const src_n = new Uint8Array(src.buffer, src.byteOffset, nbyte);
  s.buf.set(src_n, beg);
  s.off = end;
};
export const read = (s: Stream, dst: Bytes, nbyte: u32): void => {
  const beg = s.off;
  const end = beg + nbyte;
  const src_n = new Uint8Array(s.buf.buffer, s.buf.byteOffset + beg, nbyte);
  dst.set(src_n, 0);
  s.off = end;
};

export const enum Whence {
  SET = 0, // SEEK_SET
  CUR = 1, // SEEK_CUR
  END = 2, // SEEK_END
}

export const seek = (s: Stream, off: i32, whence: Whence = Whence.SET): i32 => {
  if (whence === Whence.CUR) s.off = s.off + off;
  else if (whence === Whence.END) s.off = s.view.byteLength + off;
  else s.off = off;
  return s.off;
};

export const tell = (s: Stream): i32 => s.off;
export const len = (s: Stream): i32 => s.view.byteLength;
export const rem = (s: Stream): i32 => s.view.byteLength - s.off;

export const putu8 = (s: Stream, x: u8): void => s.view.setUint8((s.off += 1) - 1, x);
export const puti8 = (s: Stream, x: i8): void => s.view.setInt8((s.off += 1) - 1, x);
export const getu8 = (s: Stream): u8 => s.view.getUint8((s.off += 1) - 1);
export const geti8 = (s: Stream): i8 => s.view.getInt8((s.off += 1) - 1);

export const putu16 = (s: Stream, x: u16): void => s.view.setUint16((s.off += 2) - 2, x, true);
export const puti16 = (s: Stream, x: i16): void => s.view.setInt16((s.off += 2) - 2, x, true);
export const getu16 = (s: Stream): u16 => s.view.getUint16((s.off += 2) - 2, true);
export const geti16 = (s: Stream): i16 => s.view.getInt16((s.off += 2) - 2, true);

export const putu32 = (s: Stream, x: u32): void => s.view.setUint32((s.off += 4) - 4, x, true);
export const puti32 = (s: Stream, x: i32): void => s.view.setInt32((s.off += 4) - 4, x, true);
export const getu32 = (s: Stream): u32 => s.view.getUint32((s.off += 4) - 4, true);
export const geti32 = (s: Stream): i32 => s.view.getInt32((s.off += 4) - 4, true);

export const putu64 = (s: Stream, x: u64): void => s.view.setBigUint64((s.off += 8) - 8, x, true);
export const puti64 = (s: Stream, x: i64): void => s.view.setBigInt64((s.off += 8) - 8, x, true);
export const getu64 = (s: Stream): u64 => s.view.getBigUint64((s.off += 8) - 8, true);
export const geti64 = (s: Stream): i64 => s.view.getBigInt64((s.off += 8) - 8, true);

export const putf32 = (s: Stream, x: f32): void => s.view.setFloat32((s.off += 4) - 4, x, true);
export const getf32 = (s: Stream): f32 => s.view.getFloat32((s.off += 4) - 4, true);

export const putf64 = (s: Stream, x: f64): void => s.view.setFloat64((s.off += 8) - 8, x, true);
export const getf64 = (s: Stream): f64 => s.view.getFloat64((s.off += 8) - 8, true);

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

export const pack96 = (lo: u32, mid: u32, hi: u32): bigint => (U64(BigInt(hi)) << 64n) | (U64(BigInt(mid)) << 32n) | U64(BigInt(lo));

// Each limb is masked while still a bigint: Number() of a value past 2^53
// rounds away the low bits before U32 could truncate them.
export const unpack96 = (x: bigint): [u32, u32, u32] => [
  Number(x & 0xffffffffn),
  Number((x >> 32n) & 0xffffffffn),
  Number((x >> 64n) & 0xffffffffn),
];

export const pack128 = (lo: u64, hi: u64): bigint => (U64(hi) << 64n) | U64(lo);

export const unpack128 = (x: bigint): [u64, u64] => [U64(x), U64(x >> 64n)];
