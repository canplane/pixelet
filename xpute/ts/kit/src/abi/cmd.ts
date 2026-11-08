// @xpute/kit/abi/cmd.ts

// A command number is `major << 8 | minor`, which fits the 16 bits a frame
// carries for it.

import type { u32 } from "@xpute/kit/abi/word.ts";

export function cmd(major: u32, minor: u32): u32 {
  return ((major & 0xff) << 8) | (minor & 0xff);
}

export function cmd_major(c: u32): u32 {
  return (c >>> 8) & 0xff;
}

export function cmd_minor(c: u32): u32 {
  return c & 0xff;
}
