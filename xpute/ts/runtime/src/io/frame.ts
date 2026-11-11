// @xpute/runtime/io/frame.ts

/** A frame's four words are laid out by spec/io/frame.json, and its packet,
 * when it has one, lies right past them. */

import type { i32, u32 } from "@xpute/kit/abi/word.ts";
import type { U8Array } from "@xpute/kit/abi/array.ts";
import * as spec from "./frame.spec.ts";

// The frame's words by name, but the command word's index, which
// frame.spec.ts keeps: `CMD` here is the command number's helper.
export {
  ACKREQ,
  FLAG_SHIFT,
  PKT,
  PROGRAM_MAJOR,
  RES,
  RESULT,
  TAG,
  WORDS,
} from "./frame.spec.ts";

const CMD_MASK: u32 = (1 << spec.FLAG_SHIFT) - 1;

export const CMD_WORD = (cmd: u32, flags: u32): u32 =>
  ((flags << spec.FLAG_SHIFT) | (cmd & CMD_MASK)) >>> 0;

export const CMD_OF = (word: u32): u32 => word & CMD_MASK;

export const FLAGS_OF = (word: u32): u32 => word >>> spec.FLAG_SHIFT;

/** A command's number is `major << 8 | minor`, which fits the 16 bits a
 * frame carries for it; its major says what answers it. */
export const CMD = (major: u32, minor: u32): u32 =>
  ((major & 0xff) << 8) | (minor & 0xff);

export const CMD_MAJOR = (cmd: u32): u32 => (cmd >>> 8) & 0xff;

export const CMD_MINOR = (cmd: u32): u32 => cmd & 0xff;

/** The bytes a frame's words take, before its packet. */
export const WORD_BYTES: u32 = spec.WORDS * 4;

/** A frame as laid in a core's memory: its words, and its packet, which lies
 * right past them. */
export interface Frame {
  tag: u32;
  cmd: u32;
  flags: u32;
  result: i32;
  pkt: U8Array | null;
}

/** The frame laid at `at`, its packet a view of the memory where it lies,
 * right past its words. */
export function frame_at(mem: ArrayBuffer, at: u32): Frame {
  const w = new Uint32Array(mem, at, spec.WORDS);
  return {
    tag: w[spec.TAG],
    cmd: CMD_OF(w[spec.CMD]),
    flags: FLAGS_OF(w[spec.CMD]),
    result: w[spec.RESULT] | 0,
    pkt:
      w[spec.PKT] === 0
        ? null
        : new Uint8Array(mem, at + WORD_BYTES, w[spec.PKT]),
  };
}

/** Whether the `len` bytes at `at` are the core's to lend: inside its memory
 * and not at offset 0, which is no core's. No bytes lend nothing, wherever. */
export function in_core(mem: ArrayBuffer, at: u32, len: u32): boolean {
  return len === 0 || (at !== 0 && at + len <= mem.byteLength);
}

/** Lays `f` at `at`, its packet right past its words. */
export function lay_frame(mem: ArrayBuffer, at: u32, f: Frame): void {
  const w = new Uint32Array(mem, at, spec.WORDS);
  w[spec.TAG] = f.tag;
  w[spec.CMD] = CMD_WORD(f.cmd, f.flags);
  w[spec.RESULT] = f.result >>> 0;
  w[spec.PKT] = f.pkt?.byteLength ?? 0;
  if (f.pkt) new Uint8Array(mem, at + WORD_BYTES, f.pkt.byteLength).set(f.pkt);
}
