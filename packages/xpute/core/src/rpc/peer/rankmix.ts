// @xpute/core/rpc/peer/rankmix.ts

/**
 * rankmix.ts — Global rank-mix ordering / selection
 *
 * Provides the global ordering primitive used by ROLE selection:
 *
 *   rank(pid) = mix * score(pid) + (1 - mix) * U(seed) + eps(seed)
 *
 * Semantics:
 *  - Higher rank is better; output is sorted by rank descending.
 *  - mix = 1: score-dominant ordering (ties broken by eps(seed))
 *  - mix = 0: deterministic random ordering (seeded)
 *  - eps(seed): tiny deterministic noise to avoid accidental ties / unstable ordering.
 *
 * Determinism:
 *
 *  - Deterministic given identical (pool enumeration order, seed, mix, score_of).
 *  - score_of(pid) must be stable for determinism guarantees upstream.
 *  - If two items produce identical rank values, final ordering may depend on input pool order
 *    due to sort stability / engine behavior.
 *
 * APIs:
 *  - rankmix_full(pool, seed, mix, score_of) -> PID[]
 *      Ranks all items, sorts globally (O(N log N)).
 *
 *  - rankmix_partial(pool, seed, mix, score_of, k) -> PID[]
 *      Computes ranks and keeps the top-k via min-heap (O(N log k)),
 *      then sorts the retained k items by rank descending.
 *
 * Notes:
 *  - This module does not dedupe; callers must provide a deduped pool if required.
 *  - “Randomness” here is deterministic noise from seed, not cryptographic entropy.
 */

import { type Element, MinHeap } from "@xpute/spec/lib/ds/heap.ts";
import { make_rng, u32_to_unit } from "@xpute/spec/lib/kit/rng.ts";

import type { PID } from "@xpute/spec/net/crypto.ts";

// Higher is better.
export type ScoreFn = (val: PID) => number;

function make_rank_of(seed: bigint, mix: number, score_of: ScoreFn) {
  const rng = make_rng(seed ^ 0x9e3779b97f4a7c15n);
  const U = () => u32_to_unit(rng());
  return (pid: PID): number => score_of(pid) * mix + U() * (1 - mix) + U() * 1e-12;
}

/**
 * Rank-mix ordering:
 *  - mix=1: score-dominant (ties broken deterministically via eps(seed))
 *  - mix=0: deterministic random ordering (seeded)
 */
export function rankmix_full(pool: PID[], seed: bigint, mix: number, score_of: ScoreFn): PID[] {
  const rank_of = make_rank_of(seed, mix, score_of);

  return pool
    .map((val): Element<PID> => ({ key: rank_of(val), val }))
    .sort((a, b) => b.key - a.key)
    .map((x) => x.val);
}

/**
 * rankmix_partial():
 *  - ranks items from an already-materialized pool
 *  - keeps top-k by rank via min-heap (O(N log k))
 *  - returns the retained k items sorted by rank descending
 */
export function rankmix_partial(pool: PID[], seed: bigint, mix: number, score_of: ScoreFn, k: number): PID[] {
  const rank_of = make_rank_of(seed, mix, score_of);

  // keep top-k largest scores => min-heap size k
  const buf: Element<PID>[] = [];
  const heap = new MinHeap<PID>(buf);

  for (let i = 0; i < pool.length; i++) {
    const val = pool[i];
    const e: Element<PID> = { key: rank_of(val), val };

    if (heap.size() < k) {
      heap.push(e);
    } else if (e.key > heap.top()!.key) {
      heap.replace_root(e);
    }
  }

  // return sorted by rank desc
  return buf.sort((a, b) => b.key - a.key).map((x) => x.val);
}
