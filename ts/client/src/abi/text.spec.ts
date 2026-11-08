// ts/client/src/abi/text.spec.ts
//
// GENERATED from spec/client/core/sys/io/text.json

/**
 * The platform's text, a device of the app's ABI beside the UI's protocol
 * (ui.json): what the core calls of it, numbered as ui.json numbers its
 * own, each a call the runtime answers beside the core before it returns
 * (ts/client/src/runtime/start.ts, port/text.ts). Where an answer lies in the
 * core's memory is laid out in spec/client/core/sys/io/text_answer.json.
 */

import type { BranchCursor } from "@xpute/kit/wire/xtp/mod.ts";
import type { f64, i32, u32 } from "@xpute/kit/abi/word.ts";
import * as Major from "./text/major.spec.ts";
import * as TextMinor from "./text/text_minor.spec.ts";
import * as Command from "./text/command.spec.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { MarshalError } from "@xpute/kit/status/error.ts";
import type { OnImport } from "@xpute/runtime";

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
  /** Its bytes. */
  text_len: u32;
}

export function read_text_layout(b: BranchCursor): TextLayout {
  return {
    weight: b.at(0).get<u32>(),
    px: b.at(1).get<f64>(),
    max_width: b.at(2).get<f64>(),
    at: b.at(3).get<u32>(),
    lines: b.at(4).get<u32>(),
    text_at: b.at(5).get<u32>(),
    text_len: b.at(6).get<u32>(),
  };
}

/** Command.TEXT_RASTER's packet. */
export interface TextRaster {
  weight: u32;
  px: f64;
  /** 0xRRGGBBAA. */
  color: u32;
  at: u32;
  width: u32;
  height: u32;
  x: f64;
  /** Where the text's UTF-8 lies in the core's memory: by address, so a text longer than a slot crosses whole, and kept until the call returns. */
  text_at: u32;
  /** Its bytes. */
  text_len: u32;
}

export function read_text_raster(b: BranchCursor): TextRaster {
  return {
    weight: b.at(0).get<u32>(),
    px: b.at(1).get<f64>(),
    color: b.at(2).get<u32>(),
    at: b.at(3).get<u32>(),
    width: b.at(4).get<u32>(),
    height: b.at(5).get<u32>(),
    x: b.at(6).get<f64>(),
    text_at: b.at(7).get<u32>(),
    text_len: b.at(8).get<u32>(),
  };
}

/** What the program does with each command the core posts on the text protocol; a
 * call's answer is its return. */
export interface TextImports {
  /** The text in the platform's own family at the weight and size, broken into lines no
   * wider than the width where it can be; at `at` in the core's memory, how many lines,
   * then a line's end (a byte offset) and its width (f32) apiece, `lines` of them at most
   * (sys/io/text_answer.json LAYOUT_ANSWER, LINE). A call: answered before it returns. */
  text_layout(e: TextLayout): i32;
  /** One line drawn in the platform's own family, starting at x with the middle of its em
   * box on the middle row, into `width` by `height` pixels at `at` in the core's memory,
   * RGBA a byte a channel, top row first, alpha not premultiplied. A call: answered before
   * it returns. */
  text_raster(e: TextRaster): i32;
}

/** `h` as the handler a runtime routes the text protocol's commands to, each one read and
 * handed to its member; ENOSYS for one the protocol does not carry. */
export function text_imports(h: TextImports): OnImport {
  return (_tag, cmd, f) => {
    switch (cmd) {
      case Command.TEXT_LAYOUT:
        return h.text_layout(read_text_layout(need(f)));
      case Command.TEXT_RASTER:
        return h.text_raster(read_text_raster(need(f)));
      default:
        return Errno.ENOSYS;
    }
  };
}

/** A packet the command carries, which a frame without one breaks. */
function need<T>(f: T | null): T {
  if (f === null) throw new MarshalError(Errno.EBADMSG);
  return f;
}
