// @xpute/runtime/ipc/frame.ts

/** A message's four words, laid out by spec/xpute/ipc/frame.json. */

import type { u32 } from "@xpute/core/abi/word.ts";
import { FLAG_SHIFT } from "./frame.spec.ts";

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
