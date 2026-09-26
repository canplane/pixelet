// @pxl/web/kernel/module.ts

/**
 * The Rust core as the shell holds it, on any platform: its memory and its
 * doorbell, which the platform opens — a page instantiates the module
 * (platform/web/core.ts), a device links the library into its app — the boot
 * block read in place, and the bodies the host holds for it. Everything else crosses in the memory,
 * where the kernel's first turn laid out the rings, `.fixed` and the heap
 * (crates/pxl/web/kernel/src/mem/section.rs).
 */

import type { U32Array, U8Array } from "@xpute/core/abi/array.ts";
import type { f64, u32 } from "@xpute/core/abi/word.ts";
import type { QuantumPolicy } from "@xpute/runtime/sched/quantum.ts";

import { BOOT_AT } from "@/kernel/mem/mem.spec.ts";

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

/** crates/pxl/web/kernel/src/door.rs's boot block, word by word: where the
 * bracket's own front ends; where the submission and completion rings are;
 * then `.fixed` and the heap, each its offset, bytes and page size; then the
 * quantum the kernel declares, three words in thousandths; then the heap's
 * row; then the two streams run after every turn, the renderer's
 * (platform/web/webgpu.ts, 0 where the core draws itself) and the host's (host.ts).
 * `data_end` is everything the bracket keeps for itself below the kernel's
 * ranges — the stack, the rings and, in a module, the linker's data. */
export const BOOT = { data_end: 0, sq: 1, cq: 2, fixed: 3, heap: 6, quantum: 9, heap_row: 12, gpu: 13, host: 14 } as const;
const BOOT_WORDS = 15;

/** The boot block, read in place at mem.json's BOOT_AT. */
export function boot_block(memory: ArrayBuffer): U32Array {
  return new Uint32Array(memory, BOOT_AT, BOOT_WORDS);
}

/** The quantum the kernel declared, for the central scheduler to grant by. */
export function quantum_policy(boot: U32Array): QuantumPolicy {
  const at = (i: u32) => boot[BOOT.quantum + i] / 1000;
  return { margin_share: at(0), batch_frames: at(1), settle_ms: at(2) };
}

/** Bodies the host holds for the kernel — a response, a model file, what
 * storage kept — until the kernel asks for them to be copied in (host.ts's
 * BODY_READ). */
const bodies = new Map<u32, U8Array>();
let next_body: u32 = 1;

/** Holds `bytes` for the kernel: the number a command names them by. */
export function hold_body(bytes: U8Array): u32 {
  const body = next_body;
  next_body = (next_body + 1) >>> 0 || 1;
  bodies.set(body, bytes);
  return body;
}

/** The body held as `body`, let go. */
export function take_body(body: u32): U8Array | undefined {
  const bytes = bodies.get(body);
  bodies.delete(body);
  return bytes;
}

const utf8_decoder = new TextDecoder();

/** The `(at, bytes)` pair at `at`: how the kernel names a run of memory it
 * built and holds, rather than carrying it through a ring slot. */
export function range_at(memory: ArrayBuffer, at: u32): readonly [u32, u32] {
  const [lies_at, bytes] = new Uint32Array(memory, at, 2);
  return [lies_at, bytes];
}

/** A range's bytes, where they lie. Valid until the kernel is given them back
 * or writes them again, so a caller that keeps them copies them first. */
export function bytes_at(memory: ArrayBuffer, at: u32, len: u32): U8Array {
  return new Uint8Array(memory, at, len);
}

/** A range's bytes as text, `len` bytes of UTF-8. */
export function text_at(memory: ArrayBuffer, at: u32, len: u32): string {
  return utf8_decoder.decode(bytes_at(memory, at, len));
}
