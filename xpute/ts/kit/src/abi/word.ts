// @xpute/kit/abi/word.ts

export type numeric = number | bigint;
export type primitive = numeric | string | boolean;

export type Vec<N extends number, T = number, R extends unknown[] = []> = R["length"] extends N ? R : Vec<N, T, [T, ...R]>;

export const BOOL = (x: unknown): 1 | 0 => x ? 1 : 0;

export type bits = 8 | 16 | 32 | 64 | 96 | 128;

export type u8 = number;
export type i8 = number;

export type u16 = number;
export type i16 = number;

export type u32 = number;
export type i32 = number;

export type u64 = bigint;
export type i64 = bigint;

export type f32 = number;
export type f64 = number;

export const U8_SZ: u32 = 1;
export const U16_SZ: u32 = 2;
export const U32_SZ: u32 = 4;
export const U64_SZ: u32 = 8;

export const U8_MAX: u32 = 0xff;
export const I8_MIN: i32 = -0x80;
export const I8_MAX: i32 = 0x7f;

export const U16_MAX: u32 = 0xffff;
export const I16_MIN: i32 = -0x8000;
export const I16_MAX: i32 = 0x7fff;

export const U32_MAX: u32 = 0xffff_ffff;
export const I32_MIN: i32 = -0x8000_0000;
export const I32_MAX: i32 = 0x7fff_ffff;

export const U64_MAX: u64 = (1n << 64n) - 1n;
export const I64_MIN: i64 = -(1n << 63n);
export const I64_MAX: i64 = (1n << 63n) - 1n;

export const is_int = (x: number): boolean => Number.isInteger(x);

export const fits_u8 = (x: number): boolean => is_int(x) && 0 <= x && x <= U8_MAX;
export const fits_i8 = (x: number): boolean => is_int(x) && I8_MIN <= x && x <= I8_MAX;
export const fits_u16 = (x: number): boolean => is_int(x) && 0 <= x && x <= U16_MAX;
export const fits_i16 = (x: number): boolean => is_int(x) && I16_MIN <= x && x <= I16_MAX;
export const fits_u32 = (x: number): boolean => is_int(x) && 0 <= x && x <= U32_MAX;
export const fits_i32 = (x: number): boolean => is_int(x) && I32_MIN <= x && x <= I32_MAX;

export const fits_u64 = (x: bigint): boolean => 0n <= x && x <= U64_MAX;
export const fits_i64 = (x: bigint): boolean => I64_MIN <= x && x <= I64_MAX;

export const fits_f32 = (x: number): boolean => Number.isFinite(x) && Math.fround(x) === x;

export const fits_f64 = (_x: number): boolean => true;

export const U8 = (u: number): u8 => u & U8_MAX;
export const I8 = (i: number): i8 => (i << 24) >> 24;
export const U16 = (u: number): u16 => u & U16_MAX;
export const I16 = (i: number): i16 => (i << 16) >> 16;

export const U32 = (u: number): u32 => u >>> 0;
export const I32 = (i: number): i32 => i | 0;

export const F32 = (f: number): f32 => Math.fround(f);

export const F64 = (f: number): f64 => f;

export const SET_BITS = (u: u32, bits: u32): u32 => U32(u | bits);
export const CLR_BITS = (u: u32, bits: u32): u32 => U32(u & ~bits);
export const HAS_BITS = (u: u32, bits: u32): boolean => (u & bits) !== 0;

export const BIT = (nbit: u32): u32 => U32(1 << nbit);
export const HAS_BIT = (u: u32, nbit: u32): boolean => HAS_BITS(u, BIT(nbit));

/** JS takes a shift count modulo 32, so a whole-word field and one past the word are special-cased, as xpute-kit's field_mask32. */
export const FIELD_MASK = (nbit: u32, shamt: u32 = 0): u32 => {
  if (shamt >= 32) return 0;
  const field = nbit >= 32 ? 0xffff_ffff : (1 << nbit) - 1;
  return U32(field << shamt);
};
export const FIELD_GET = (u: u32, shamt: u32, mask: u32): u32 => U32((u >>> shamt) & mask);
export const FIELD_SET = (u: u32, shamt: u32, mask: u32, v: u32): u32 => U32((u & ~(mask << shamt)) | ((v & mask) << shamt));

export const U64 = (x: bigint): u64 => x & U64_MAX;
export const I64 = (x: bigint): i64 => BigInt.asIntN(64, x);

export const SET_BITS64 = (x: u64, bits: u64): u64 => U64(x | bits);
export const CLR_BITS64 = (x: u64, bits: u64): u64 => U64(x & ~bits);
export const HAS_BITS64 = (x: u64, bits: u64): boolean => (x & bits) !== 0n;

export const BIT64 = (shamt: u32): u64 => U64(1n << BigInt(shamt));
export const HAS_BIT64 = (x: u64, shamt: u32): boolean => HAS_BITS64(x, BIT64(shamt));

export const FIELD_MASK64 = (nbit: u32, shamt: u32 = 0): u64 => U64(((1n << BigInt(nbit)) - 1n) << BigInt(shamt));
export const FIELD_GET64 = (x: u64, shamt: u32, mask: u64): u64 => U64((x >> BigInt(shamt)) & mask);
export const FIELD_SET64 = (x: u64, shamt: u32, mask: u64, v: u64): u64 => U64((x & ~(mask << BigInt(shamt))) | ((v & mask) << BigInt(shamt)));
