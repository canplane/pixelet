// @pxl/native/platform/core.ts

/**
 * The kernel as a device runs it: the Rust core linked into the app
 * (crates/pxl/web/native), reached through the app's kernel module
 * (modules/pxl-surface/ios/PxlKernelModule.swift) — its memory as one buffer
 * over the region the core took, and its doorbell. The same two things a page
 * gets from instantiating the module, so everything past this is the same.
 */

import type { f64, u32 } from "@xpute/core/abi/word.ts";
import { requireNativeModule } from "expo";
import type { Core } from "@/kernel/module.ts";

/** The app's kernel module: the core's memory and doorbell, and on this side
 * of the platform alone, its device and the surfaces it draws into. */
export interface PxlKernel {
  memory(): ArrayBuffer;
  interrupt(quota_ms: f64): f64;
  /** The device's uniform offset alignment and whether it times its passes,
   * or null where the system has no GPU. */
  gpuOpen(): [u32, u32] | null;
  /** Binds a kernel surface to the world view's layer: false when no view has
   * left one yet. */
  surfaceBind(handle: u32): boolean;
  surfaceUnbind(handle: u32): void;
}

export const PXL_KERNEL: PxlKernel = requireNativeModule<PxlKernel>("PxlKernel");

/** The core, booted: its first turn lays the memory out before anything reads
 * the boot block. */
export function open_core(): Promise<Core> {
  const buffer = PXL_KERNEL.memory();
  PXL_KERNEL.interrupt(0);
  return Promise.resolve({ buffer, interrupt: (quota_ms: f64) => PXL_KERNEL.interrupt(quota_ms) });
}
