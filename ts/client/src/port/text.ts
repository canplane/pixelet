// @pxl/client/port/text.ts

/**
 * TEXT on the shell's port: the kernel's calls TEXT_LAYOUT and TEXT_RASTER,
 * answered from the platform's text before they return, so what they write
 * in the kernel's memory is in place when the call returns.
 */

import type { BranchCursor } from "@xpute/kit/wire/xtp/mod.ts";
import { td, te } from "@xpute/kit/codec/encoding.ts";

import { read_text_layout, read_text_raster } from "@/abi/cmd.spec.ts";
import { LAYOUT_ANSWER, LINE } from "@/abi/host.spec.ts";
import { bracket } from "@/bracket.tsx";
import { HOST } from "@/host/mount.ts";

/** The text a call names in the kernel's memory, held until the call returns. */
function text_of(at: number, len: number): string {
  return td.decode(new Uint8Array(HOST.guest.mem as ArrayBuffer, at, len));
}

/** What no line starts with: closing marks, and in Japanese the small kana and
 * the marks that lean on the character before. */
const NO_LINE_START =
  /[、。，．・：；？！）］｝〕〉》」』】”’…‥ーぁぃぅぇぉっゃゅょゎァィゥェォッャュョヮゝゞヽヾ々,.:;!?)\]}]/;

/** What no line ends with: opening marks. */
const NO_LINE_END = /[（［｛〔〈《「『【“‘(\[{]/;

/** The break at `at` moved back past a mark the rules keep with its neighbor,
 * no further than the line's first character. */
function kept_break(text: string, line: number, at: number): number {
  while (
    at - 1 > line &&
    (NO_LINE_START.test(text[at]) || NO_LINE_END.test(text[at - 1])) &&
    !isSurrogate(text.charCodeAt(at - 1))
  ) {
    at -= 1;
  }
  return at;
}

function isSurrogate(unit: number): boolean {
  return unit >= 0xd800 && unit <= 0xdfff;
}

/** Greedy by words; a word too wide alone (Korean, Japanese or Chinese typed
 * without spaces) breaks between characters. A space stays at the next line's
 * head for the kernel to trim. */
function line_ends(
  text: string,
  max_width: number,
  width: (s: string) => number,
): number[] {
  const words = [...text.matchAll(/\S+/g)].map((m) => [
    m.index,
    m.index + m[0].length,
  ]);
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
          chunk = kept_break(text, chunk, i);
          ends.push(chunk);
        }
        i = next;
      }
    }
    ends.push(e);
  }
  ends[ends.length - 1] = text.length;
  return ends;
}

/** TEXT_LAYOUT: the text laid out a line at a time, each line's end and
 * width written at the call's `at`. */
export function text_layout(f: BranchCursor): void {
  const { weight, px, max_width, at, lines, text_at, text_len } =
    read_text_layout(f);
  const text = text_of(text_at, text_len);
  const width = (s: string) => bracket.text.width(weight, px, s);
  const ends = line_ends(text, max_width, width).slice(0, lines);
  const out = new DataView(
    HOST.guest.mem as ArrayBuffer,
    at,
    4 * (LAYOUT_ANSWER.WORDS + LINE.WORDS * lines),
  );
  out.setUint32(4 * LAYOUT_ANSWER.cnt, ends.length, true);
  let start = 0;
  let start_byte = 0;
  ends.forEach((end, i) => {
    const line = text.slice(start, end);
    start_byte += te.encode(line).length;
    const l = LAYOUT_ANSWER.WORDS + LINE.WORDS * i;
    out.setUint32(4 * (l + LINE.end), start_byte, true);
    out.setFloat32(4 * (l + LINE.width), width(line.trim()), true);
    start = end;
  });
}

/** TEXT_RASTER: one line's pixels written at the call's `at`. */
export function text_raster(f: BranchCursor): void {
  const { weight, px, color, at, width, height, x, text_at, text_len } =
    read_text_raster(f);
  const text = text_of(text_at, text_len);
  bracket.text.raster(
    weight,
    px,
    color,
    text,
    x,
    width,
    height,
    new Uint8Array(HOST.guest.mem as ArrayBuffer, at, width * height * 4),
  );
}
