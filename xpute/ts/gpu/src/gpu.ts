// @xpute/gpu/gpu.ts

/**
 * What a core asks of its GPU, and the settings of the runtime that serves it
 * (web.ts). @xpute/runtime knows none of it.
 */

import type { u32 } from "@xpute/kit/abi/word.ts";

/** This is what a core asks of its GPU (its manifest's `gpu`), by WebGPU's
 * names. */
export interface GpuAsk {
  power: string;
  features: { required: readonly string[]; optional: readonly string[] };
}

/** These are the settings of a page's GPU runtime that the program chooses,
 * as xpute-gpu's `CoreConfig` gives a device's runtime its own. */
export interface GpuConfig {
  /** The size of one chunk that buffer writes are staged through (belt.ts),
   * as large as one frame's writes; a larger write is staged across several
   * chunks. */
  upload_chunk_bytes: u32;
  /** What the runtime does on the thread that runs it that takes longer than
   * this many milliseconds is said, as `[gpu] <what> took <ms> ms`, the whole
   * milliseconds: a record of the stream (`op <op>`, by its number), the
   * stream's run as a whole (`run`), and a completion's landing: a read's
   * (`map`), a staging chunk's mapped again (`remap`) and an image's
   * (`image`). With null, nothing is timed. */
  slow_ms: u32 | null;
}

/** What a page has of a GPU, as the runtime reports it: one, none from the
 * adapter, none since one was lost while the core asks again, none in this
 * browser, or none in a page that is not a secure context. */
export type GpuState = "present" | "missing" | "waiting" | "unsupported" | "insecure";

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
