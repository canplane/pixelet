// @xpute/core/abi/cmd.ts

/**
 * A command's number: a major that names what it is about, and a minor that
 * names what is done to it — `major << 8 | minor` in the 16 bits a message
 * carries (ipc/frame's word 1). Numbers, never strings: a table is an index,
 * and a string was the part certain to change. What the majors and minors
 * are is the program's, not this kit's.
 */

import type { u32 } from "@xpute/core/abi/word.ts";

export function cmd(major: u32, minor: u32): u32 {
  return ((major & 0xff) << 8) | (minor & 0xff);
}

export function cmd_major(c: u32): u32 {
  return (c >>> 8) & 0xff;
}

export function cmd_minor(c: u32): u32 {
  return c & 0xff;
}
