// script/vector/pxl/shared/spec/space/pvf3/pvf3.ts
//
// Every operation of ivec, fvec and pvf3 over a fixed set of words — the
// lane extremes, zero, one, and a seeded scatter — as the TypeScript
// computes it, for crates/pxl/spec/src/space/pvf3/pvf3.test.rs to
// reproduce bit for bit. A result that throws in the TypeScript (a BigInt
// division by zero) is recorded as "throw".

import { fvec, ivec, type PVF3, pvf3 } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { f64_bits, seeded } from "../../../../../../vector.ts";

const S = (x: bigint) => x.toString();

function try_(f: () => bigint | boolean | string | number): unknown {
  try {
    const v = f();
    return typeof v === "bigint" ? S(v) : typeof v === "number" ? f64_bits(v) : v;
  } catch {
    return "throw";
  }
}

export function vectors() {
  const rnd = seeded(0x5eed);
  const r64 = () => (BigInt(rnd() >>> 0) << 32n) | BigInt(rnd() >>> 0);
  const iwords: bigint[] = [
    0n,
    1n,
    ivec.fill(1n),
    ivec.fill(-1n),
    ivec.gather(-(1n << 23n), (1n << 23n) - 1n, -(1n << 15n)),
    ivec.gather((1n << 23n) - 1n, -(1n << 23n), (1n << 15n) - 1n),
    0xffff_ffff_ffff_ffffn,
  ];
  for (let k = 0; k < 24; k++) iwords.push(r64());
  const fwords: bigint[] = [0n, 1n, fvec.fill(255n), fvec.fill(128n), 0xff_ffffn];
  for (let k = 0; k < 16; k++) fwords.push(r64() & 0xffff_ffffn);
  const shifts = [-40, -24, -17, -8, -1, 0, 1, 7, 8, 16, 23];
  const reals = [0, 0.5, -0.5, -0.2, 1, -1, 1.999, -1.999, 12345.678, -12345.678, 1e-9, -1e-9, 8388607.99, -8388608];

  const unary = (m: typeof ivec | typeof fvec, w: bigint) => ({
    w: S(w),
    X: S(m.X(w)),
    Y: S(m.Y(w)),
    Z: S(m.Z(w)),
    repr: m.repr(w),
    not: S(m.not(w)),
    sh: shifts.map((k) => try_(() => m.sh(w, k))),
    max_comp: S(m.max_comp(w)),
    min_comp: S(m.min_comp(w)),
    sum: S(m.sum(w)),
    norm1: S(m.norm1(w)),
    norm_sq: S(m.norm_sq(w)),
    ...(m === ivec ? { neg: S(ivec.neg(w)), abs: S(ivec.abs(w)) } : {}),
  });
  const binary = (m: typeof ivec | typeof fvec, v: bigint, w: bigint) => ({
    v: S(v),
    w: S(w),
    and: S(m.and(v, w)),
    or: S(m.or(v, w)),
    xor: S(m.xor(v, w)),
    add: S(m.add(v, w)),
    sub: S(m.sub(v, w)),
    mul: S(m.mul(v, w)),
    div: try_(() => m.div(v, w)),
    mod: try_(() => m.mod(v, w)),
    eq: S(m.eq(v, w)),
    equals: m.equals(v, w),
    lt: S(m.lt(v, w)),
    gt: S(m.gt(v, w)),
    max: S(m.max(v, w)),
    min: S(m.min(v, w)),
    clamp: S(m.clamp(v, -5n, 300n)),
    dot: S(m.dot(v, w)),
    dist1: S(m.dist1(v, w)),
    dist_sq: S(m.dist_sq(v, w)),
  });
  const pairs = <T>(xs: T[]) => xs.flatMap((v, i) => xs.slice(0, 8).map((w) => [v, w] as const).filter((_, j) => (i + j) % 3 === 0));

  const tuples: PVF3[] = iwords.slice(0, 16).map((i, k) => ({ i, f: fwords[k % fwords.length] }));
  const P = (p: PVF3) => ({ i: S(p.i), f: S(p.f) });
  const out = (): PVF3 => ({ i: 0n, f: 0n });

  return {
    ivec: {
      unary: iwords.map((w) => unary(ivec, w)),
      binary: pairs(iwords).map(([v, w]) => binary(ivec, v, w)),
      gather: [[-1n, 2n, -3n], [1n << 30n, -(1n << 30n), 1n << 20n], [-(1n << 63n), (1n << 63n) - 1n, 0n]].map(([x, y, z]) => ({ x: S(x), y: S(y), z: S(z), w: S(ivec.gather(x, y, z)) })),
      from_real: reals.map((r) => ({ r: f64_bits(r), v: S(ivec.from_real(r)) })),
      to_real: [0n, -1n, 1n << 40n, -(1n << 52n) - 3n].map((c) => ({ c: S(c), r: f64_bits(ivec.to_real(c)) })),
    },
    fvec: {
      unary: fwords.map((w) => unary(fvec, w)),
      binary: pairs(fwords).map(([v, w]) => binary(fvec, v, w)),
      from_real: reals.map((r) => ({ r: f64_bits(r), v: S(fvec.from_real(r)) })),
      to_real: [0n, 1n, 128n, 255n, 256n, -1n].map((c) => ({ c: S(c), r: f64_bits(fvec.to_real(c)) })),
    },
    pvf3: {
      unary: tuples.map((p) => ({
        p: P(p),
        X: S(pvf3.X(p)),
        Y: S(pvf3.Y(p)),
        Z: S(pvf3.Z(p)),
        repr: pvf3.repr(p),
        not: P(pvf3.not(out(), p)),
        sh: shifts.map((k) => {
          try {
            return P(pvf3.sh(out(), p, k));
          } catch {
            return "throw";
          }
        }),
        neg: P(pvf3.neg(out(), p)),
        abs: P(pvf3.abs(out(), p)),
        sum: S(pvf3.sum(p)),
        norm1: S(pvf3.norm1(p)),
        norm_sq: S(pvf3.norm_sq(p)),
        max_comp: S(pvf3.max_comp(p)),
        min_comp: S(pvf3.min_comp(p)),
      })),
      binary: pairs(tuples).map(([v, w]) => {
        const t = (f: () => PVF3) => {
          try {
            return P(f());
          } catch {
            return "throw";
          }
        };
        return {
          v: P(v),
          w: P(w),
          and: P(pvf3.and(out(), v, w)),
          or: P(pvf3.or(out(), v, w)),
          xor: P(pvf3.xor(out(), v, w)),
          add: P(pvf3.add(out(), v, w)),
          sub: P(pvf3.sub(out(), v, w)),
          mul: P(pvf3.mul(out(), v, w)),
          div: t(() => pvf3.div(out(), v, w)),
          mod: t(() => pvf3.mod(out(), v, w)),
          eq: P(pvf3.eq(out(), v, w)),
          equals: pvf3.equals(v, w),
          lt: P(pvf3.lt(out(), v, w)),
          gt: P(pvf3.gt(out(), v, w)),
          max: P(pvf3.max(out(), v, w)),
          min: P(pvf3.min(out(), v, w)),
          clamp: P(pvf3.clamp(out(), v, -5000n, 70000n)),
          dot: S(pvf3.dot(v, w)),
          dist1: S(pvf3.dist1(v, w)),
          dist_sq: S(pvf3.dist_sq(v, w)),
        };
      }),
      gather: [[-1n, 2n, -3n], [(1n << 31n) + 77n, -(1n << 31n) - 5n, 1n << 23n], [-(1n << 62n), (1n << 62n) - 1n, 12345n]].map(([x, y, z]) => ({
        x: S(x),
        y: S(y),
        z: S(z),
        p: P(pvf3.gather(out(), x, y, z)),
      })),
      from_real: reals.map((r) => ({ r: f64_bits(r), v: S(pvf3.from_real(r)) })),
      to_real: [0n, 1n, -1n, 255n, 1n << 40n].map((c) => ({ c: S(c), r: f64_bits(pvf3.to_real(c)) })),
    },
  };
}
