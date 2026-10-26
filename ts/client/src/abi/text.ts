// @pxl/client/abi/text.ts

/**
 * The kernel's own system calls, TEXT_LAYOUT and TEXT_RASTER, answered from
 * the platform's text. They name no root: the program numbers them past
 * xpute's PROGRAM bit, and they run between turns with xpute's, so what they
 * write is in place by the time the kernel reads it.
 */

import type { U32Array } from "@xpute/kit/abi/array.ts";
import type { u32 } from "@xpute/kit/abi/word.ts";
import { te } from "@xpute/kit/codec/encoding.ts";
import { text_of } from "@xpute/host/ipc/stream.ts";

import { op } from "@/abi/host.spec.ts";
import { bracket } from "@/bracket.tsx";

const word = new DataView(new ArrayBuffer(4));

function float(bits: u32): number {
  word.setUint32(0, bits, true);
  return word.getFloat32(0, true);
}

/** Greedy by words; a word too wide alone (Korean typed without spaces)
 * breaks between characters. A space stays at the next line's head for the
 * kernel to trim. */
function line_ends(
  text: string,
  max_width: number,
  width: (s: string) => number,
): number[] {
  const words = [...text.matchAll(/\S+/g)].map((
    m,
  ) => [m.index, m.index + m[0].length]);
  if (max_width <= 0 || words.length === 0) return [text.length];
  const spans: [number, number][] = [];
  let [start, end] = words[0];
  for (let i = 1; i < words.length; i++) {
    const [ws, we] = words[i];
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

/** A call of the program's own (host.json `op`): the text the kernel lays
 * out and rasters, a line at a time, widths then pixels. */
export function text_call(code: u32, w: U32Array, a: u32): void {
  switch (code) {
    case op.TEXT_LAYOUT: {
      const [weight, px, max_width, at, lines] = [
        w[a],
        float(w[a + 1]),
        float(w[a + 2]),
        w[a + 3],
        w[a + 4],
      ];
      const text = text_of(w, a + 5);
      const width = (s: string) => bracket.text.width(weight, px, s);
      const ends = line_ends(text, max_width, width).slice(0, lines);
      const out = new DataView(w.buffer, at, 4 * (1 + 2 * lines));
      out.setUint32(0, ends.length, true);
      let start = 0;
      let start_byte = 0;
      ends.forEach((end, i) => {
        const line = text.slice(start, end);
        start_byte += te.encode(line).length;
        out.setUint32(4 * (1 + 2 * i), start_byte, true);
        out.setFloat32(4 * (2 + 2 * i), width(line.trim()), true);
        start = end;
      });
      break;
    }
    case op.TEXT_RASTER: {
      const [weight, px, rgba, at, width, height, x] = [
        w[a],
        float(w[a + 1]),
        w[a + 2],
        w[a + 3],
        w[a + 4],
        w[a + 5],
        float(w[a + 6]),
      ];
      bracket.text.raster(
        weight,
        px,
        rgba,
        text_of(w, a + 7),
        x,
        width,
        height,
        new Uint8Array(w.buffer, at, width * height * 4),
      );
      break;
    }
    default:
      throw new Error(`[host] call ${code}`);
  }
}
