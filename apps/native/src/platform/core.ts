// @pxl/native/platform/core.ts

/**
 * The kernel as a device runs it: the Rust core linked into the app
 * (crates/pxl/web/native), reached through the app's kernel module
 * (modules/pxl-surface/ios/PxlKernelModule.swift) — its memory as one buffer
 * over the region the core took, and its doorbell. The same two things a page
 * gets from instantiating the module, so everything past this is the same.
 */

import type { f64 } from "@xpute/core/abi/word.ts";
import { requireNativeModule } from "expo";
import type { Core } from "@/kernel/module.ts";

interface PxlKernel {
  memory(): ArrayBuffer;
  interrupt(quota_ms: f64): f64;
}

/** The core, booted: its first turn lays the memory out before anything reads
 * the boot block. */
export function open_core(): Promise<Core> {
  const kernel = requireNativeModule<PxlKernel>("PxlKernel");
  const buffer = kernel.memory();
  kernel.interrupt(0);
  return Promise.resolve({ buffer, interrupt: (quota_ms: f64) => kernel.interrupt(quota_ms) });
}
