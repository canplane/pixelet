// @pxl/web/kernel/host.ts

/**
 * What the kernel asks of the host that no command answers (crates/pxl/web/
 * kernel/src/host.rs): a body copied in, a value stored, a text laid out or
 * drawn (crates/pxl/web/kernel/src/text.rs), a broken invariant. Each is a record in the host stream (abi/host.json), run
 * here once the turn has returned and before the next can begin, so whatever
 * a record names in the kernel's memory is written by the time the kernel
 * reads it.
 */

import type { u32 } from "@xpute/core/abi/word.ts";
import { strerror } from "@xpute/core/status/errno.spec.ts";
import { bytes_of, each_record } from "@xpute/runtime/ipc/stream.ts";

import { op, STORAGE_KEYS, stream } from "@/kernel/abi/host.spec.ts";
import { take_body, text_at } from "@/kernel/module.ts";
import { storage_get, storage_set } from "@platform/storage.ts";
import { text_raster, text_width } from "@platform/text.ts";

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
  return storage_get(STORAGE_KEYS[storage]);
}

/** Runs the records the turn just returned left in the host stream at `at`. */
export function run_host_stream(memory: ArrayBuffer, at: u32): void {
  each_record(memory, at, stream.HEAD, (code, w, a) => {
    switch (code) {
      case op.BODY_READ: {
        const [body, dst, len] = [w[a], w[a + 1], w[a + 2]];
        const bytes = take_body(body);
        if (bytes !== undefined && len > 0) new Uint8Array(memory, dst, len).set(bytes.subarray(0, len));
        break;
      }
      case op.STORAGE_SET:
        storage_set(STORAGE_KEYS[w[a]], text_at(memory, w[a + 1], w[a + 2]));
        break;
      case op.TEXT_LAYOUT: {
        const [weight, px, max_width, at, lines] = [w[a], float(w[a + 1]), float(w[a + 2]), w[a + 3], w[a + 4]];
        const text = utf8.decode(bytes_of(w, a + 5));
        const width = (s: string) => text_width(weight, px, s);
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
        text_raster(weight, px, rgba, utf8.decode(bytes_of(w, a + 7)), x, width, height, new Uint8Array(memory, at, width * height * 4));
        break;
      }
      case op.PANIC: {
        const [errno, line, col] = [w[a] | 0, w[a + 1], w[a + 2]];
        const num = (i: u32) => BigInt(w[a + i]) | BigInt(w[a + i + 1]) << 32n;
        const file = utf8.decode(bytes_of(w, a + 7)).replace(/^\.+/, "");
        console.error(`[pxl] ${strerror(errno)} at ${file}:${line}:${col} (${num(3)}, ${num(5)})`);
        break;
      }
      default:
        throw new Error(`[host] record ${code}`);
    }
  });
}
