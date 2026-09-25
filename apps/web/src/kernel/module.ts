// @pxl/web/kernel/module.ts

/**
 * The Rust core's module (crates/pxl/wasm). The bytes are built by `deno task
 * build:wasm` into `kernel.wasm` beside this file and are not committed; Vite
 * hashes them into the bundle through the `new URL` below, and Deno's tests
 * read them through the same URL.
 *
 * The module imports its memory: the central scheduler gives it one
 * (mem.ts), and its first turn lays out everything inside it — the rings,
 * `.fixed` and the heap — as the kernel's map says
 * (crates/pxl/web/kernel/src/mem/section.rs). Past the memory it imports the
 * clock alone and exports the doorbell alone: everything else crosses in the
 * memory.
 *
 * One instance per memory: instantiating writes the module's data segments
 * over the front of it, so a second instance over the same memory would
 * erase the first's state — its heap's free lists among it.
 *
 * Instantiated from bytes rather than streamed: streaming insists on an
 * `application/wasm` response, which neither a `file:` URL under Deno nor
 * every static host gives.
 */

import type { U32Array, U8Array } from "@xpute/core/abi/array.ts";
import type { f64, u32 } from "@xpute/core/abi/word.ts";
import type { QuantumPolicy } from "@xpute/runtime/sched/quantum.ts";

import { BOOT_AT } from "@/kernel/mem/mem.spec.ts";
import { DEV_BUNDLE } from "@/config/bundle.ts";

/** What the module exports: the doorbell. */
export interface KernelExports {
  memory: WebAssembly.Memory;
  /** Hands the kernel a turn of `quota_ms`; when it wants the next, in ms
   * (@xpute/runtime/sched/quantum.ts). The first turn lays the memory out. */
  interrupt(quota_ms: f64): f64;
}

/** crates/pxl/web/wasm/src/lib.rs's boot block, word by word: where the module's
 * own front ends; where the submission and completion rings are; then `.fixed`
 * and the heap, each its offset, bytes and page size; then the quantum the
 * kernel declares, three words in thousandths; then the heap's row; then the
 * two streams run after every turn, the renderer's (render/gpu.ts) and the
 * host's (host.ts). `data_end` is where the linker stopped, which is
 * everything the module keeps for itself — the stack, the rings and its own
 * data. */
export const BOOT = { data_end: 0, sq: 1, cq: 2, fixed: 3, heap: 6, quantum: 9, heap_row: 12, gpu: 13, host: 14 } as const;
const BOOT_WORDS = 15;

/** The boot block, read in place at mem.json's BOOT_AT. */
export function boot_block(pxl: KernelExports): U32Array {
  return new Uint32Array(pxl.memory.buffer, BOOT_AT, BOOT_WORDS);
}

/** The quantum the kernel declared, for the central scheduler to grant by. */
export function quantum_policy(boot: U32Array): QuantumPolicy {
  const at = (i: u32) => boot[BOOT.quantum + i] / 1000;
  return { margin_share: at(0), batch_frames: at(1), settle_ms: at(2) };
}

const KERNEL_WASM_URL = new URL("./kernel.wasm", import.meta.url);

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
export function range_at(memory: WebAssembly.Memory, at: u32): readonly [u32, u32] {
  const [lies_at, bytes] = new Uint32Array(memory.buffer, at, 2);
  return [lies_at, bytes];
}

/** A range's bytes, where they lie. Valid until the kernel is given them back
 * or writes them again, so a caller that keeps them copies them first.
 *
 * The core memory is reserved here and never shared, so the cast holds; what
 * it buys is a view the platform accepts where an `ArrayBuffer` is asked for. */
export function bytes_at(memory: WebAssembly.Memory, at: u32, len: u32): U8Array {
  return new Uint8Array(memory.buffer as ArrayBuffer, at, len);
}

/** A range's bytes as text, `len` bytes of UTF-8. */
export function text_at(memory: WebAssembly.Memory, at: u32, len: u32): string {
  return utf8_decoder.decode(bytes_at(memory, at, len));
}

/** Instantiates the module over the memory the central scheduler gives it,
 * and boots it. */
export async function instantiate_kernel(memory: WebAssembly.Memory): Promise<KernelExports> {
  // Not from the HTTP cache in development: the dev server rebuilds the
  // module under the same URL, and a cached one silently runs yesterday's
  // core under today's glue.
  const bytes = await (await fetch(KERNEL_WASM_URL, DEV_BUNDLE ? { cache: "no-store" } : undefined)).arrayBuffer();
  const { instance } = await WebAssembly.instantiate(bytes, {
    // The clock a tick's budget is spent against: the one call the module
    // makes, since a turn measures itself while the host cannot run.
    env: { memory, now: () => performance.now() },
  });
  const exports: KernelExports = { interrupt: instance.exports.interrupt as KernelExports["interrupt"], memory };
  // A turn with no quota applies what waits, which is nothing: the first one
  // lays the memory out and writes the boot block.
  exports.interrupt(0);
  return exports;
}
