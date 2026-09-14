// @pxl/web/wasm/pxl.ts

/**
 * The Rust core's module (crates/pxl/wasm), instantiated over the core
 * memory. The bytes are built by `deno task build:wasm` into `pxl.wasm`
 * beside this file and are not committed; Vite hashes them into the bundle
 * through the `new URL` below, and Deno's tests read them through the same
 * URL.
 *
 * The module imports its memory rather than owning one, and keeps the
 * front of it — the map's module section — for its own stack, data and
 * heap. It says how much it keeps, and instantiation fails here rather
 * than later if the map disagrees: the sections after it would be under
 * the module's stack.
 *
 * One instance per memory, and for the core memory it is core_pool.ts's,
 * made before anything else uses the memory: instantiating writes the
 * module's data segments over the module section, so a second instance
 * over the same memory would erase the first's state — its pools' free
 * lists among it.
 *
 * Instantiated from bytes rather than streamed: streaming insists on an
 * `application/wasm` response, which neither a `file:` URL under Deno nor
 * every static host gives.
 */

import type { u32 } from "@xpute/core/kit/word.ts";

/** What the module exports: the boot block and the doorbell. Every address
 * is into the memory it was instantiated over. */
export type PxlExports = {
  memory: WebAssembly.Memory;
  /** The boot block's address. */
  boot(): u32;
  /** Hands the kernel the turn for `vector` (@xpute/core/ipc/cmd.ts's
   * Interrupt). */
  interrupt(vector: u32): u32;
};

/** crates/pxl/wasm/src/lib.rs's boot block, word by word: the section the
 * module keeps, and where the submission and completion rings are. */
export const BOOT = { module_bytes: 0, sq: 1, cq: 2 } as const;
const BOOT_WORDS = 3;

/** The boot block, read in place. */
export function boot_block(pxl: PxlExports): Uint32Array {
  return new Uint32Array(pxl.memory.buffer, pxl.boot(), BOOT_WORDS);
}

const PXL_WASM_URL = new URL("./pxl.wasm", import.meta.url);

const utf8 = new TextEncoder();
const utf8_decoder = new TextDecoder();

function text_at(memory: WebAssembly.Memory, at: u32, len: u32): string {
  return utf8_decoder.decode(new Uint8Array(memory.buffer, at, len));
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

/** Instantiates the module over `memory`, whose first `module_bytes` are
 * the map's module section. */
export async function instantiate_pxl(memory: WebAssembly.Memory, module_bytes: u32): Promise<PxlExports> {
  // Not from the HTTP cache in development: the dev server rebuilds the
  // module under the same URL, and a cached one silently runs yesterday's
  // core under today's glue.
  const bytes = await (await fetch(PXL_WASM_URL, import.meta.env?.DEV ? { cache: "no-store" } : undefined)).arrayBuffer();
  const { instance } = await WebAssembly.instantiate(bytes, {
    env: {
      memory,
      // A panic's message, before the trap: what "unreachable" would otherwise hide.
      panic: (ptr: u32, len: u32) => console.error(`[pxl] ${new TextDecoder().decode(new Uint8Array(memory.buffer, ptr, len))}`),
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
    },
  });
  const exports: PxlExports = { ...(instance.exports as unknown as PxlExports), memory };
  const keeps = boot_block(exports)[BOOT.module_bytes];
  if (keeps !== module_bytes) {
    throw new Error(`pxl: the module keeps ${keeps} bytes of the memory and the map reserves ${module_bytes}`);
  }
  return exports;
}
