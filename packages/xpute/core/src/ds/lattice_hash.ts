// @xpute/core/ds/lattice_hash.ts

/**
 * One hash for an integer lattice address, shared by every structure that
 * buckets cells: the spatial hash's broadphase and the billboard lattice's
 * nodes. Teschner's three large primes decorrelate the axes before the xor,
 * so a lattice of points does not collapse onto a few buckets; a fourth
 * folds in a tier, for a table that holds several tiers of one lattice at
 * once, where a parent's address would otherwise collide with its child's.
 */

import type { i32, u32 } from "../kit/word.ts";

const P1 = 73856093;
const P2 = 19349663;
const P3 = 83492791;
const P4 = 32452843;

export function lattice_hash3(x: i32, y: i32, z: i32): u32 {
  return (Math.imul(x, P1) ^ Math.imul(y, P2) ^ Math.imul(z, P3)) >>> 0;
}

export function lattice_hash4(x: i32, y: i32, z: i32, tier: u32): u32 {
  return (Math.imul(x, P1) ^ Math.imul(y, P2) ^ Math.imul(z, P3) ^ Math.imul(tier, P4)) >>> 0;
}
