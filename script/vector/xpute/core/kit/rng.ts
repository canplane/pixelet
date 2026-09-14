// script/vector/xpute/core/kit/rng.ts
//
// The seeded streams and seed derivations, for
// crates/xpute/core/src/kit/rng.test.rs.

import { derive_seed_u64, make_rng, mix64, u32_to_unit } from "@xpute/core/kit/rng.ts";
import { f64_bits } from "../../../../vector.ts";

export function vectors() {
  const seeds = [0n, 1n, 12345n, 0x9e3779b97f4a7c15n, (1n << 64n) - 1n, 0xd0c0dec05eed0001n];
  return seeds.map((seed) => {
    const rng = make_rng(seed);
    const draws = Array.from({ length: 12 }, () => rng());
    return { seed, mix64: mix64(seed), derive: derive_seed_u64(seed, 77n), draws, unit: draws.map((d) => f64_bits(u32_to_unit(d))) };
  });
}
