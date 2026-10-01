// @pxl/web/kernel/module.ts

/**
 * The Rust core as the shell holds it, on any platform: its memory and its
 * doorbell, which the platform opens — a page instantiates the module
 * (platform/web/core.ts), a device links the library into its app — its
 * directory read in place. Everything else crosses in the memory,
 * where the kernel's first turn laid out the rings, `.fixed` and the heap
 * (crates/pxl/web/kernel/src/mem/section.rs).
 */

import type { U8Array } from "@xpute/core/abi/array.ts";
import type { f64, u32 } from "@xpute/core/abi/word.ts";
import { Directory } from "@xpute/runtime/ipc/directory.ts";

import { DIRECTORY_AT } from "@/kernel/mem/mem.spec.ts";

/** The core, opened and booted: its memory, which never moves or grows, and
 * its doorbell. */
export interface Core {
  buffer: ArrayBuffer;
  /** Hands the kernel a turn of `quota_ms`; when it wants the next, in ms
   * (@xpute/runtime/sched/quantum.ts). The first turn lays the memory out. A
   * platform whose core records its GPU calls runs them here, before the
   * turn is over, so what it drew is on the device before anything else can
   * reach it. */
  interrupt(quota_ms: f64): f64;
}

/** The directory crates/pxl/web/kernel/src/door.rs writes on the first turn,
 * at mem.json's DIRECTORY_AT: xpute's entries — the rings, the system-call
 * stream the host runs after every turn (host.ts) and the quantum the kernel
 * declares — and the kernel's own (abi/directory.spec.ts): the renderer's
 * stream, also run after every turn (platform/web/webgpu.ts, 0 where the core
 * draws itself), and the heap's row. */
export function directory(memory: ArrayBuffer): Directory {
  return new Directory(memory, DIRECTORY_AT);
}

/** A range's bytes, where they lie. Valid until the kernel is given them back
 * or writes them again, so a caller that keeps them copies them first. */
export function bytes_at(memory: ArrayBuffer, at: u32, len: u32): U8Array {
  return new Uint8Array(memory, at, len);
}
