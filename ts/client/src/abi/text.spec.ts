// ts/client/src/abi/text.spec.ts
//
// GENERATED from spec/client/core/sys/io/text.json

/**
 * The platform's text, a device of the app's ABI beside the UI's protocol
 * (ui.json): what the core calls of it, numbered as ui.json numbers its
 * own, each a call answered before it returns: on a page by the runtime
 * (ts/client/src/service/core/text.ts), on a device on the core's own thread
 * (pxl-core sys/io/text_device.rs). Where an answer lies in the
 * core's memory is laid out in spec/client/core/sys/io/text_answer.json.
 */

import type { BranchCursor } from "@xpute/kit/wire/xtp/mod.ts";
import type { f64, u32 } from "@xpute/kit/abi/word.ts";
import { FINITE, INT_U32 } from "@xpute/runtime/abi/number.ts";
import * as Major from "./text/major.spec.ts";
import * as TextMinor from "./text/text_minor.spec.ts";
import * as Command from "./text/command.spec.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { MarshalError } from "@xpute/kit/status/error.ts";
import type { Protocol, OnImport } from "@xpute/runtime";
import { refusal_of } from "@xpute/runtime";

/** One of Major's, as its enum was a type. */
type Major = (typeof Major)[keyof typeof Major];
export { Major };

/** One of TextMinor's, as its enum was a type. */
type TextMinor = (typeof TextMinor)[keyof typeof TextMinor];
export { TextMinor };

/** One of Command's, as its enum was a type. */
type Command = (typeof Command)[keyof typeof Command];
export { Command };

/** Command.TEXT_LAYOUT's packet. */
export interface TextLayout {
  weight: u32;
  px: f64;
  /** 0 for one line. */
  max_width: f64;
  at: u32;
  lines: u32;
  /** Where the text's UTF-8 lies in the core's memory: by address, so a text longer than a slot crosses whole, and kept until the call returns. */
  text_at: u32;
  /** The length of that UTF-8 in bytes. */
  text_len: u32;
}

/** Command.TEXT_RASTER's packet. */
export interface TextRaster {
  weight: u32;
  px: f64;
  /** The color as 0xRRGGBBAA. */
  color: u32;
  at: u32;
  width: u32;
  height: u32;
  x: f64;
  /** Where the text's UTF-8 lies in the core's memory: by address, so a text longer than a slot crosses whole, and kept until the call returns. */
  text_at: u32;
  /** The length of that UTF-8 in bytes. */
  text_len: u32;
}

/** Each import's packet, read from its branch, by its number. */
export const read = {
  [Command.TEXT_LAYOUT]: (b: BranchCursor): TextLayout => ({
    weight: INT_U32(b.at(0).get_number()),
    px: FINITE(b.at(1).get_number()),
    max_width: FINITE(b.at(2).get_number()),
    at: INT_U32(b.at(3).get_number()),
    lines: INT_U32(b.at(4).get_number()),
    text_at: INT_U32(b.at(5).get_number()),
    text_len: INT_U32(b.at(6).get_number()),
  }),
  [Command.TEXT_RASTER]: (b: BranchCursor): TextRaster => ({
    weight: INT_U32(b.at(0).get_number()),
    px: FINITE(b.at(1).get_number()),
    color: INT_U32(b.at(2).get_number()),
    at: INT_U32(b.at(3).get_number()),
    width: INT_U32(b.at(4).get_number()),
    height: INT_U32(b.at(5).get_number()),
    x: FINITE(b.at(6).get_number()),
    text_at: INT_U32(b.at(7).get_number()),
    text_len: INT_U32(b.at(8).get_number()),
  }),
};

/** The majors the runtime routes the protocol's imports by. */
export const MAJORS: readonly u32[] = [Major.TEXT];

/** What the program does with each command the core posts on the text protocol: a
 * call's member answers its value, and throws kit's error to refuse it. */
export interface TextImports {
  /** The text in the platform's own family at the weight and size, broken into lines no
   * wider than the width where it can be; at `at` in the core's memory, how many lines,
   * then a line's end (a byte offset) and its width (f32) apiece, `lines` of them at most
   * (sys/io/text_answer.json LAYOUT_ANSWER, LINE). A call: answered before it returns. */
  [Command.TEXT_LAYOUT](e: TextLayout): u32;
  /** One line drawn in the platform's own family, starting at x with the middle of its em
   * box on the middle row, into `width` by `height` pixels at `at` in the core's memory,
   * RGBA a byte a channel, top row first, alpha not premultiplied. A call: answered before
   * it returns. */
  [Command.TEXT_RASTER](e: TextRaster): u32;
}

/** `h` as the protocol the runtime routes the text protocol's majors to: each
 * command read and handed to its member, whose refusal is answered with its
 * errno; ENOSYS for one the protocol does not carry. */
export function text_imports(h: TextImports): Protocol {
  const on: OnImport = (_tag, cmd, f) => {
    try {
      switch (cmd) {
        case Command.TEXT_LAYOUT:
          return h[Command.TEXT_LAYOUT](read[Command.TEXT_LAYOUT](need(f)));
        case Command.TEXT_RASTER:
          return h[Command.TEXT_RASTER](read[Command.TEXT_RASTER](need(f)));
        default:
          return Errno.ENOSYS;
      }
    } catch (err) {
      return refusal_of(err);
    }
  };
  return { majors: MAJORS, on };
}

/** A packet the command carries, which a frame without one breaks. */
function need<T>(f: T | null): T {
  if (f === null) throw new MarshalError(Errno.EBADMSG);
  return f;
}
