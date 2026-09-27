// @pxl/native/platform/core.ts

/**
 * The kernel as a device runs it: the Rust core linked into the app
 * (crates/pxl/web/native), reached through the app's native module
 * (module.ts) — its memory as one buffer
 * over the region the core took, and its doorbell. The same two things a page
 * gets from instantiating the module, so everything past this is the same.
 */

import type { f64 } from "@xpute/core/abi/word.ts";
import type { Core } from "@/kernel/module.ts";
import { PLATFORM } from "./module.ts";

/** A device the app runs on is a phone, a tablet or a headset, held to a
 * phone's ceilings — the memory the core takes is a phone's too
 * (crates/pxl/web/native/src/memory.rs `GIVEN`). */
export const HANDHELD: boolean = true;

/** The core, booted: its first turn lays the memory out before anything reads
 * the boot block. */
export function openCore(): Promise<Core> {
  const buffer = PLATFORM.memory();
  PLATFORM.interrupt(0);
  return Promise.resolve({ buffer, interrupt: (quotaMs: f64) => PLATFORM.interrupt(quotaMs) });
}
