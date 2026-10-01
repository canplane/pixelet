// @pxl/web/kernel/host.ts

/**
 * The kernel's system calls as its host answers them (crates/pxl/web/kernel/
 * src/host.rs): xpute's, run by xpute (@xpute/runtime/ipc/sys.ts), with a file
 * written to the platform's storage and a broken invariant said on the
 * console; and the kernel's own (abi/host.json), a text laid out or drawn
 * (crates/pxl/web/kernel/src/text.rs). All of them run once the turn has
 * returned and before the next can begin, so whatever one names in the
 * kernel's memory is written by the time the kernel reads it.
 */

import type { U32Array } from "@xpute/core/abi/array.ts";
import type { u32 } from "@xpute/core/abi/word.ts";
import { strerror } from "@xpute/core/status/errno.spec.ts";
import { bytes_of } from "@xpute/runtime/ipc/stream.ts";
import { run_sys, type SysHost } from "@xpute/runtime/ipc/sys.ts";

import { op, STORAGE_KEYS } from "@/kernel/abi/host.spec.ts";
import { BODIES } from "@/kernel/module.ts";
import { storageGet, storageSet } from "@platform/storage.ts";
import { textRaster, textWidth } from "@platform/text.ts";

const utf8 = new TextDecoder();

const word = new DataView(new ArrayBuffer(4));

/** A word's bits as a float. */
function float(bits: u32): number {
  word.setUint32(0, bits, true);
  return word.getFloat32(0, true);
}

const utf8_out = new TextEncoder();

/** Where `text` breaks into lines no wider than `max_width` (0 for one
 * line), measured by `width`: each line's end, a UTF-16 index, the last the
 * text's end. Greedy by words; a line still too wide on its own — one long
 * word, or a message typed without spaces, as Korean often is — breaks
 * between characters. A break at a space leaves the space at the head of the
 * next line, for the kernel to trim. */
function line_ends(text: string, max_width: number, width: (s: string) => number): number[] {
  const words = [...text.matchAll(/\S+/g)].map((m) => [m.index, m.index + m[0].length]);
  if (max_width <= 0 || words.length === 0) return [text.length];
  const spans: [number, number][] = [];
  let [start, end] = words[0];
  for (const [ws, we] of words.slice(1)) {
    if (width(text.slice(start, we)) <= max_width) end = we;
    else {
      spans.push([start, end]);
      [start, end] = [ws, we];
    }
  }
  spans.push([start, end]);
  const ends: number[] = [];
  for (const [s, e] of spans) {
    if (width(text.slice(s, e)) > max_width) {
      let chunk = s;
      for (let i = s; i < e;) {
        const next = i + (text.codePointAt(i)! > 0xffff ? 2 : 1);
        if (i > chunk && width(text.slice(chunk, next)) > max_width) {
          ends.push(i);
          chunk = i;
        }
        i = next;
      }
    }
    ends.push(e);
  }
  ends[ends.length - 1] = text.length;
  return ends;
}

/** What storage keeps under `storage`'s key (host.json), or none. */
export function stored(storage: u32): string | null {
  return storageGet(STORAGE_KEYS[storage]);
}

/** The kernel's own calls: a text laid out, or drawn. */
function program(memory: ArrayBuffer, code: u32, w: U32Array, a: u32): void {
  switch (code) {
    case op.TEXT_LAYOUT: {
      const [weight, px, max_width, at, lines] = [w[a], float(w[a + 1]), float(w[a + 2]), w[a + 3], w[a + 4]];
      const text = utf8.decode(bytes_of(w, a + 5));
      const width = (s: string) => textWidth(weight, px, s);
      const ends = line_ends(text, max_width, width).slice(0, lines);
      const out = new DataView(memory, at, 4 * (1 + 2 * lines));
      out.setUint32(0, ends.length, true);
      let start = 0;
      ends.forEach((end, i) => {
        out.setUint32(4 * (1 + 2 * i), utf8_out.encode(text.slice(0, end)).length, true);
        out.setFloat32(4 * (2 + 2 * i), width(text.slice(start, end).trim()), true);
        start = end;
      });
      break;
    }
    case op.TEXT_RASTER: {
      const [weight, px, rgba, at, width, height, x] = [w[a], float(w[a + 1]), w[a + 2], w[a + 3], w[a + 4], w[a + 5], float(w[a + 6])];
      textRaster(weight, px, rgba, utf8.decode(bytes_of(w, a + 7)), x, width, height, new Uint8Array(memory, at, width * height * 4));
      break;
    }
    default:
      throw new Error(`[host] call ${code}`);
  }
}

/** Runs the calls the turn just returned left in the stream at `at`. */
export function run_host_stream(memory: ArrayBuffer, at: u32): void {
  const host: SysHost = {
    write: (file, bytes) => storageSet(STORAGE_KEYS[file], utf8.decode(bytes)),
    abort: ({ errno, file, line, column, a, b }) => console.error(`[pxl] ${strerror(errno)} at ${file.replace(/^\.+/, "")}:${line}:${column} (${a}, ${b})`),
    program: (code, w, a) => program(memory, code, w, a),
  };
  run_sys(memory, at, BODIES, host);
}
