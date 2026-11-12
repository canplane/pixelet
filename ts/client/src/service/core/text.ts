// @pxl/client/service/core/text.ts

/**
 * The text the runtime serves the core (spec/client/core/wit/text.wit), from
 * the platform's text. Its methods answer at once, so what they write into
 * the core's memory is in place by the time the call returns.
 */

import { td, te } from "@xpute/kit/codec/encoding.ts";
import type { Link } from "@xpute/runtime";

import * as text from "@/abi/text.ts";
import { LAYOUT_ANSWER, LINE } from "@/abi/text_answer.spec.ts";
import * as font from "@/service/platform/text.ts";

export function serve_text(link: Link): void {
  /** The text a call names in the core's memory, which holds it only until
   * the call returns. */
  const text_of = (at: number, len: number): string => td.decode(link.bytes(at, len));

  text.mount(link, {
    /** A line's end counts UTF-8 bytes, and its width leaves out the spaces
     * at its ends. */
    layout(weight, px, max_width, at, lines, text_at, text_len) {
      const s = text_of(text_at, text_len);
      const width = (t: string) => font.width(weight, px, t);
      const ends = line_ends(s, max_width, width).slice(0, lines);
      const b = link.bytes(at, 4 * (LAYOUT_ANSWER.WORDS + LINE.WORDS * lines));
      const out = new DataView(b.buffer, b.byteOffset, b.byteLength);
      out.setUint32(4 * LAYOUT_ANSWER.cnt, ends.length, true);
      let start = 0;
      let start_byte = 0;
      ends.forEach((end, i) => {
        const line = s.slice(start, end);
        start_byte += te.encode(line).length;
        const l = LAYOUT_ANSWER.WORDS + LINE.WORDS * i;
        out.setUint32(4 * (l + LINE.end), start_byte, true);
        out.setFloat32(4 * (l + LINE.width), width(line.trim()), true);
        start = end;
      });
      return 0;
    },
    raster(weight, px, color, at, width, height, x, text_at, text_len) {
      const s = text_of(text_at, text_len);
      font.raster(weight, px, color, s, x, width, height, link.bytes(at, width * height * 4));
      return 0;
    },
  });
}

/** The characters no line may start with: closing marks and, in Japanese, the
 * small kana and the marks that lean on the character before them. */
const NO_LINE_START =
  /[、。，．・：；？！）］｝〕〉》」』】”’…‥ーぁぃぅぇぉっゃゅょゎァィゥェォッャュョヮゝゞヽヾ々,.:;!?)\]}]/;

/** The characters no line may end with: opening marks. */
const NO_LINE_END = /[（［｛〔〈《「『【“‘([{]/;

/** Moves the break at `at` back past any mark that NO_LINE_START or
 * NO_LINE_END keeps with its neighbor, but never past the line's first
 * character. */
function allowed_break(text: string, line: number, at: number): number {
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

/** Words fill each line greedily. A word too wide for a line by itself, as
 * Korean, Japanese or Chinese typed without spaces can be, breaks between
 * characters. A space stays at the head of the next line, for the core to
 * trim. */
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
          chunk = allowed_break(text, chunk, i);
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
