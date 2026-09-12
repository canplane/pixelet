// @pxl/web/module/world/lod_build_stat.ts

/**
 * Cumulative (since page load) LOD build timing, dev-only — no console.log
 * anywhere near this (a hot console during a build storm would itself skew
 * the measurement); just plain object writes each call, read by the
 * scheduler's frame sample and the ledger report (sched/log.ts).
 */
export const lod_build_stats = {
  scan_ms: 0,
  scan_count: 0,
  /** Of scan_count, how many took the direct path (runs from the layers'
   * sources, no volume) rather than a scan of a composite. */
  direct_count: 0,
  scan_max_ms: 0,
  scan_bands: 0,
  scan_bands_max: 0,
  build_ms: 0,
  build_count: 0,
  build_max_ms: 0,
  build_runs: 0,
  build_runs_max: 0,
};
