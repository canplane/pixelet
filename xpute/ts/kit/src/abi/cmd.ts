// @xpute/kit/abi/cmd.ts

/** `major << 8 | minor` in the 16 bits ipc/frame's word 1 carries. */

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
