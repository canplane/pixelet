// @pxl/web/kernel/host.ts

/**
 * What the kernel asks of the host that no command answers (crates/pxl/web/
 * kernel/src/host.rs): a body copied in, a value stored, glyphs measured, a
 * panic's message. Each is a record in the host stream (abi/host.json), run
 * here once the turn has returned and before the next can begin, so whatever
 * a record names in the kernel's memory is written by the time the kernel
 * reads it.
 */

import type { u32 } from "@xpute/core/abi/word.ts";
import { bytes_of, each_record } from "@xpute/runtime/ipc/stream.ts";

import { op, STORAGE_KEYS, stream } from "@/kernel/abi/host.spec.ts";
import { take_body, text_at } from "@/kernel/module.ts";
import { css_font } from "@/kernel/render/gpu.ts";

const utf8 = new TextDecoder();

let measure: OffscreenCanvasRenderingContext2D | null | undefined;

/** A 2D context to measure text with, where the runtime has one. */
function measure_context(): OffscreenCanvasRenderingContext2D | null {
  if (measure === undefined) measure = typeof OffscreenCanvas === "undefined" ? null : new OffscreenCanvas(1, 1).getContext("2d");
  return measure;
}

const word = new DataView(new ArrayBuffer(4));

/** What the browser's storage keeps under `storage`'s key (host.json), or
 * none. */
export function stored(storage: u32): string | null {
  try {
    return globalThis.localStorage?.getItem(STORAGE_KEYS[storage]) ?? null;
  } catch {
    return null;
  }
}

/** Runs the records the turn just returned left in the host stream at `at`. */
export function run_host_stream(memory: WebAssembly.Memory, at: u32): void {
  each_record(memory.buffer, at, stream.HEAD, (code, w, a, n) => {
    switch (code) {
      case op.BODY_READ: {
        const [body, dst, len] = [w[a], w[a + 1], w[a + 2]];
        const bytes = take_body(body);
        if (bytes !== undefined && len > 0) new Uint8Array(memory.buffer, dst, len).set(bytes.subarray(0, len));
        break;
      }
      case op.STORAGE_SET:
        try {
          globalThis.localStorage?.setItem(STORAGE_KEYS[w[a]], text_at(memory, w[a + 1], w[a + 2]));
        } catch {
          // Storage refused (a private window's quota): the kernel keeps its state for the session.
        }
        break;
      case op.MEASURE: {
        word.setUint32(0, w[a + 1], true);
        const ctx = measure_context();
        if (ctx !== null) ctx.font = css_font(w[a], word.getFloat32(0, true));
        const out = new Float32Array(memory.buffer, w[a + 2], n - 3);
        for (let i = 0; i < out.length; i++) out[i] = ctx === null ? 7 : ctx.measureText(String.fromCodePoint(w[a + 3 + i])).width;
        break;
      }
      case op.PANIC:
        console.error(`[pxl] ${utf8.decode(bytes_of(w, a))}`);
        break;
      default:
        throw new Error(`[host] record ${code}`);
    }
  });
}
