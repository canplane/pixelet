// @xpute/runtime/io/frame.ts

/** A frame's four words are laid out by spec/io/frame.json, and its packet,
 * when it has one, lies right past them. */

import type { i32, u32 } from "@xpute/kit/abi/word.ts";
import type { U8Array } from "@xpute/kit/abi/array.ts";
import { CMD, FLAG_SHIFT, PKT, RESULT, TAG, WORDS } from "./frame.spec.ts";

export * from "./frame.spec.ts";

const CMD_MASK: u32 = (1 << FLAG_SHIFT) - 1;

export function cmd_word(cmd: u32, flags: u32): u32 {
  return ((flags << FLAG_SHIFT) | (cmd & CMD_MASK)) >>> 0;
}

export function cmd_of(word: u32): u32 {
  return word & CMD_MASK;
}

export function flags_of(word: u32): u32 {
  return word >>> FLAG_SHIFT;
}

/** A command's number lies in a major, which says what answers it. */
export function major_of(cmd: u32): u32 {
  return (cmd >>> 8) & 0xff;
}

/** The bytes a frame's words take, before its packet. */
export const WORD_BYTES: u32 = WORDS * 4;

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
  const w = new Uint32Array(mem, at, WORDS);
  return {
    tag: w[TAG],
    cmd: cmd_of(w[CMD]),
    flags: flags_of(w[CMD]),
    result: w[RESULT] | 0,
    pkt: w[PKT] === 0 ? null : new Uint8Array(mem, at + WORD_BYTES, w[PKT]),
  };
}

/** Whether the `len` bytes at `at` are the core's to lend: inside its memory
 * and not at offset 0, which is no core's. No bytes lend nothing, wherever. */
export function in_core(mem: ArrayBuffer, at: u32, len: u32): boolean {
  return len === 0 || (at !== 0 && at + len <= mem.byteLength);
}

/** Lays `f` at `at`, its packet right past its words. */
export function lay_frame(mem: ArrayBuffer, at: u32, f: Frame): void {
  const w = new Uint32Array(mem, at, WORDS);
  w[TAG] = f.tag;
  w[CMD] = cmd_word(f.cmd, f.flags);
  w[RESULT] = f.result >>> 0;
  w[PKT] = f.pkt?.byteLength ?? 0;
  if (f.pkt) new Uint8Array(mem, at + WORD_BYTES, f.pkt.byteLength).set(f.pkt);
}
