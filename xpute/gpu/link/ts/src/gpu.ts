// @xpute/gpu-link/gpu.ts

/**
 * The GPU's words both its hosts take: what a guest asks of its GPU, the
 * settings its program chooses for the GPU's host, and what the host has of
 * a GPU (xpute-gpu-link lib.rs).
 */

import type { u32 } from "@xpute/kit/abi/word.ts";
import { uri } from "@xpute/link";

/** The URI the guest opens its GPU device by (gpu/wit/gpu.wit `device`),
 * which the GPU's hosts mount (xpute-gpu-link lib.rs). */
export const DEVICE_URI = uri("xpute:gpu", "device");

/** This is what a guest asks of its GPU, as its program's data spec says, by
 * WebGPU's names: the power preference, the features the device must have,
 * and those it takes where the adapter has them. */
export interface GpuAsk {
  power: string;
  required_feats: readonly string[];
  optional_feats: readonly string[];
}

/** These are the settings of the GPU's host that the program chooses. The
 * frame latency, which the guest admits frames by and a device's surface
 * queues, is not one a page's host reads: its browser paces its frames. */
export interface GpuConfig {
  /** The size of one chunk that buffer writes are staged through, as large
   * as one frame's writes; a larger write is staged across several chunks. */
  upload_chunk_bytes: u32;
  /** What the host does on the thread that runs it that takes longer than
   * this many milliseconds is logged, as `[gpu] <what> took <ms> ms`, the whole
   * milliseconds: a record of the stream (`op <op>`, by its number), the
   * stream's run as a whole (`run`), and a completion's landing: a read's
   * (`map`), a staging chunk's mapped again (`remap`) and an image's
   * (`image`). With null, nothing is timed. */
  slow_ms: u32 | null;
}

/** What the GPU's host has of a GPU, as it reports it: one, none from the
 * adapter, none since one was lost while the guest asks again, none on this
 * platform, or none in a page that is not a secure context. */
export type GpuState =
  "present" | "missing" | "waiting" | "unsupported" | "insecure";
