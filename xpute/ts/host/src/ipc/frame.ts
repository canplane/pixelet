// @xpute/host/ipc/frame.ts

/** A frame's four words, laid out by spec/ipc/frame.json. */

import type { u32 } from "@xpute/kit/abi/word.ts";
import { PORT_SHIFT, FLAG_SHIFT } from "./frame.spec.ts";

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

/** The far port a frame's tag names. */
export function port_of(tag: u32): u32 {
  return tag >>> PORT_SHIFT;
}

/** A tag's id or subject, below its port. */
export function id_of(tag: u32): u32 {
  return tag & ((1 << PORT_SHIFT) - 1);
}

/** The tag of a frame of `port`'s, its id or subject `id`. */
export function tagged(port: u32, id: u32): u32 {
  return ((port << PORT_SHIFT) | id) >>> 0;
}
