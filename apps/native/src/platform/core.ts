// @pxl/native/platform/core.ts

/** The Rust core linked into the app, reached through the native module: a memory buffer and a doorbell, as a page gets. */

import type { f64 } from "@xpute/kit/abi/word.ts";
import type { Core } from "@/kernel/module.ts";
import { PLATFORM } from "./module.ts";

/** Every device the app runs on is held to a phone's ceilings. */
export const HANDHELD: boolean = true;

export function openCore(): Promise<Core> {
  const buffer = PLATFORM.memory();
  PLATFORM.interrupt(0);
  return Promise.resolve({ buffer, interrupt: (quotaMs: f64) => PLATFORM.interrupt(quotaMs) });
}
