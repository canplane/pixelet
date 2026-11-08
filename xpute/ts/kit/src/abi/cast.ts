// @xpute/kit/abi/cast.ts

// Reads the low `w` bits of a value as unsigned or as two's-complement signed.
// `u` and `d` work on a number, with `w` from 1 to 32 because JavaScript's bit
// operators are 32-bit; `lu` and `ld` work on a bigint.

export type nbits = number;

export const u = (x: number, w: nbits): number => {
  if (w === 32) return x >>> 0;
  return x & ((1 << w) - 1);
};

export const d = (x: number, w: nbits): number => {
  if (w === 32) return x | 0;

  const m = (1 << w) - 1;
  const s = 1 << (w - 1);

  x &= m;
  return x & s ? x | ~m : x;
};

export const lu = (x: bigint, w: nbits): bigint => {
  const W = BigInt(w);
  return x & ((1n << W) - 1n);
};

export const ld = (x: bigint, w: nbits): bigint => {
  const W = BigInt(w);
  const m = (1n << W) - 1n;
  const s = 1n << (W - 1n);

  x &= m;
  return x & s ? x - (1n << W) : x;
};
