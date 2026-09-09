// @pxl/web/modules/world/sched/categories.ts

/**
 * Static per-category classification — set once from profiling, read by the
 * dev overlay and (later) the findings analysis, never fed back into
 * scheduling. This is the "arithmetic intensity / bound-class" row of
 * discussions/20260909-cooperative-scheduler-and-task-ledger.md: it says
 * which optimization lever a hotspot in this category needs, not how fast it
 * currently is.
 *
 * - network   → prefetch + priority; a faster kernel does nothing
 * - memory    → touch less data / overlap on a worker; WASM+SIMD barely helps
 * - compute   → WASM + SIMD pays here
 * - gpu-fill  → fewer post-fx passes / lower dpr / less overdraw
 */

export type BoundClass = "network" | "memory" | "compute" | "gpu-fill";

export type CategoryInfo = {
  bound: BoundClass;
  /** one line on the lever this bound implies for this category. */
  note: string;
};

const CATEGORIES: Record<string, CategoryInfo> = {
  terrain: {
    bound: "network",
    note:
      "Measured 2026-09-09: 4.2MB/page, mean net 322.5ms vs decode 0.6ms — decode_terrain is an XTP cursor read and is free. The demand is network-bound end to end. mobile-perf bottleneck A is NOT in here; that cascade (page_to_chunks/dense_ele/decode_ground_rgba) hangs off set_tile inside React's commit, and shows up as chunk_stage's commit_ms, not as this category's run time.",
  },
  poi: {
    bound: "network",
    note:
      "Measured 2026-09-09: mean net 269.3ms vs JSON.parse 2.2ms on ~114KB pages (295KB worst). Latency-bound; lever is priority order, not parallelism.",
  },
  geocode: {
    bound: "network",
    note: "single-slot latest-wins; one small page lookup, pure RTT. Already debounced upstream, so the lever left is queue priority.",
  },
  search: {
    bound: "network",
    note:
      "the one feature with no static bake — always a live API round trip, and the user is watching. Runs ahead of every streaming category on purpose.",
  },
};

export function category_info(category: string): CategoryInfo | undefined {
  return CATEGORIES[category];
}
