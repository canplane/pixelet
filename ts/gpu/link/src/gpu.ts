// @pxl/gpu/link/src/gpu.ts

/**
 * The GPU's words both its hosts take: what a guest asks of its GPU, the
 * settings its program chooses for the GPU's host, and what the host has of
 * a GPU (pxl-gpu-link lib.rs).
 */

import type { u32 } from "@xpute/kit/abi/word.ts";

/** This is what a guest asks of its GPU, as its program's data spec says, by
 * WebGPU's names: the power preference, the features the device must have,
 * and those it takes where the adapter has them. */
export interface GpuAsk {
  power: string;
  required_feats: readonly string[];
  optional_feats: readonly string[];
}

/** These are the settings of the GPU's host that the program chooses, the
 * same on a page and a device (pxl-gpu-link `GpuConfig`). */
export interface GpuConfig {
  /** How many frames may be admitted and unfinished on the GPU at once, at
   * least one. The host tells it the guest in the device's block (spec
   * DEVICE), which the guest admits frames by (pxl-gpu-guest
   * `admit::frame`); a page's browser paces the frames themselves. */
  frame_latency: u32;
  /** The size of one chunk that buffer writes are staged through, as large
   * as one frame's writes; a larger write is staged across several chunks. */
  upload_chunk_sz: u32;
}

/** What the GPU's host has of a GPU, as it reports it: one, none from the
 * adapter, none since one was lost while the guest asks again, none on this
 * platform, or none in a page that is not a secure context. */
export type GpuState =
  "present" | "missing" | "waiting" | "unsupported" | "insecure";
