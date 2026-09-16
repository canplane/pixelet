// @pxl/web/wasm/module.ts

/**
 * The Rust core's module (crates/pxl/wasm). The bytes are built by `deno task
 * build:wasm` into `kernel.wasm` beside this file and are not committed; Vite
 * hashes them into the bundle through the `new URL` below, and Deno's tests
 * read them through the same URL.
 *
 * The module imports its memory: the central scheduler gives it one
 * (core_mem.ts), and booting the module lays out everything inside it —
 * the sections, their pools, the rings — as the kernel's map says
 * (crates/pxl/core/src/mem.rs).
 *
 * One instance per memory: instantiating writes the module's data segments
 * over the front of it, so a second instance over the same memory would
 * erase the first's state — its pools' free lists among it.
 *
 * Instantiated from bytes rather than streamed: streaming insists on an
 * `application/wasm` response, which neither a `file:` URL under Deno nor
 * every static host gives.
 */

import type { u32 } from "@xpute/core/kit/word.ts";
import type { QuantumPolicy } from "@xpute/core/sched/quantum.ts";

import { gpu_imports } from "@/wasm/gpu.ts";

/** What the module exports: the boot block and the doorbell. Every address
 * is into the memory it was given. */
export type KernelExports = {
  memory: WebAssembly.Memory;
  /** Lays out the memory the module was given, the first time; the boot
   * block's address. */
  boot(): u32;
  /** Hands the kernel the turn for `vector` (@xpute/core/ipc/cmd.ts's
   * Interrupt). */
  interrupt(vector: u32): u32;
  /** The gauge panel's numbers over the last `window_ms`: the address of two
   * words, where the JSON lies in this memory and how long it is. */
  ledger_gauge(window_ms: number): u32;
  /** Says the panel's numbers have been read, so the kernel can drop them. */
  ledger_gauge_release(): void;
  /** What the ledger's live stream has kept since the last read: the address
   * of two words, where the JSON lies in this memory and how long it is, zero
   * when nothing is waiting. A range and not a command because instrumentation
   * does not belong on the control ring. */
  ledger_log(): u32;
  /** Says the stream's records have been read, so the kernel can drop them. */
  ledger_log_release(): void;
  /** The ledger's trace: the address of two words, where the Trace Event
   * Format JSON lies in this memory and how long it is. A range and not a
   * message because a window's trace runs to tens of megabytes. */
  ledger_trace(): u32;
  /** Says the trace has been copied, so the kernel can give its bytes back. */
  ledger_trace_release(): void;
};

/** crates/pxl/wasm/src/lib.rs's boot block, word by word: where the module's
 * own front ends; where the submission and completion rings are; then `.fixed`
 * and the heap, each its offset, bytes and page size; then the quantum the
 * kernel declares, five words in thousandths; then the heap's row. */
/** Where each thing the kernel reports lies in the boot block. `data_end` is
 * where the linker stopped, which is everything the module keeps for itself —
 * the stack, the rings and its own data. */
export const BOOT = { data_end: 0, sq: 1, cq: 2, fixed: 3, heap: 6, quantum: 9, heap_row: 14 } as const;
const BOOT_WORDS = 15;

/** The boot block, read in place. */
export function boot_block(pxl: KernelExports): Uint32Array {
  return new Uint32Array(pxl.memory.buffer, pxl.boot(), BOOT_WORDS);
}

/** The quantum the kernel declared, for the central scheduler to grant by. */
export function quantum_policy(boot: Uint32Array): QuantumPolicy {
  const at = (i: number) => boot[BOOT.quantum + i] / 1000;
  return { margin_share: at(0), min_ms: at(1), max_ms: at(2), batch_ms: at(3), outside_ema: at(4) };
}

const KERNEL_WASM_URL = new URL("./kernel.wasm", import.meta.url);

const utf8 = new TextEncoder();
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
export function bytes_at(memory: WebAssembly.Memory, at: u32, len: u32): Uint8Array<ArrayBuffer> {
  return new Uint8Array(memory.buffer as ArrayBuffer, at, len);
}

/** A range's bytes as text, `len` bytes of UTF-8. */
export function text_at(memory: WebAssembly.Memory, at: u32, len: u32): string {
  return utf8_decoder.decode(bytes_at(memory, at, len));
}

function storage_read(key: string): string | null {
  try {
    return globalThis.localStorage?.getItem(key) ?? null;
  } catch {
    return null;
  }
}

let measure: OffscreenCanvasRenderingContext2D | null | undefined;

/** A 2D context to measure text with, where the runtime has one. */
function measure_context(): OffscreenCanvasRenderingContext2D | null {
  if (measure === undefined) measure = typeof OffscreenCanvas === "undefined" ? null : new OffscreenCanvas(1, 1).getContext("2d");
  return measure;
}

/** Instantiates the module over the memory the central scheduler gives it,
 * and boots it. */
export async function instantiate_kernel(memory: WebAssembly.Memory): Promise<KernelExports> {
  // Not from the HTTP cache in development: the dev server rebuilds the
  // module under the same URL, and a cached one silently runs yesterday's
  // core under today's glue.
  const bytes = await (await fetch(KERNEL_WASM_URL, import.meta.env?.DEV ? { cache: "no-store" } : undefined)).arrayBuffer();
  const { instance } = await WebAssembly.instantiate(bytes, {
    env: {
      memory,
      // A panic's message, before the trap: what "unreachable" would otherwise hide.
      panic: (ptr: u32, len: u32) => console.error(`[pxl] ${text_at(memory, ptr, len)}`),
      // The clock a tick's budget is spent against.
      now: () => performance.now(),
      // The kernel's storage (crates/pxl/core/src/host.rs): an item's UTF-8
      // length, or -1 for none, then its bytes into the kernel's buffer.
      storage_len: (key: u32, key_len: u32) => {
        const value = storage_read(text_at(memory, key, key_len));
        return value === null ? -1 : utf8.encode(value).byteLength;
      },
      storage_get: (key: u32, key_len: u32, dst: u32, cap: u32) => {
        const value = storage_read(text_at(memory, key, key_len));
        if (value === null) return;
        const bytes = utf8.encode(value).subarray(0, cap);
        new Uint8Array(memory.buffer, dst, bytes.byteLength).set(bytes);
      },
      storage_set: (key: u32, key_len: u32, value: u32, value_len: u32) => {
        try {
          globalThis.localStorage?.setItem(text_at(memory, key, key_len), text_at(memory, value, value_len));
        } catch {
          // Storage refused (a private window's quota): the kernel keeps its state for the session.
        }
      },
      // The local wall clock: unix ms, the local hour, minute and second, and
      // the local day's start.
      local_time: (out: u32) => {
        const d = new Date();
        const start = new Date(d);
        start.setHours(0, 0, 0, 0);
        new Float64Array(memory.buffer, out, 5).set([d.getTime(), d.getHours(), d.getMinutes(), d.getSeconds(), start.getTime()]);
      },
      measure_text: (font: u32, font_len: u32, text: u32, text_len: u32) => {
        const ctx = measure_context();
        const s = text_at(memory, text, text_len);
        if (ctx === null) return s.length * 7;
        ctx.font = text_at(memory, font, font_len);
        return ctx.measureText(s).width;
      },
      entropy: () => crypto.getRandomValues(new Uint32Array(1))[0],
      // The renderer's WebGPU (crates/pxl/core/src/render/gpu/api.rs).
      ...gpu_imports(memory),
    },
  });
  const exports: KernelExports = { ...(instance.exports as unknown as KernelExports), memory };
  exports.boot();
  return exports;
}
