// @xpute/gpu-web-host/timed.ts

/** The page's host's measure of its own work against the program's
 * `slow_ms` (@xpute/gpu-link `GpuConfig`). */

import type { u32 } from "@xpute/kit/abi/word.ts";

/** Runs `f`, and where `slow_ms` is not null and `f` took longer, says
 * `[gpu] <what> took <ms> ms`, the whole milliseconds (`GpuConfig.slow_ms`). */
export function timed<T>(slow_ms: u32 | null, what: string, f: () => T): T {
  if (slow_ms === null) return f();
  const t0 = performance.now();
  const v = f();
  const ms = Math.trunc(performance.now() - t0);
  if (ms > slow_ms) console.warn(`[gpu] ${what} took ${ms} ms`);
  return v;
}
