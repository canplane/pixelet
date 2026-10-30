// @xpute/host/ipc/toc.spec.ts
//
// GENERATED from spec/ipc/toc.json
//
// The toc: where a host finds what its guest laid out in the memory. The guest's
// first turn writes it, and the host reads it once, at the one offset it knows outside
// the memory (xpute-guest ipc/toc.rs, and the contract's rule 0 in
// ARCHITECTURE.md).
//
// Its words: MAGIC, VERSION, how many entries follow, and a word kept at 0; then that
// many entries, a key and a value each, no key twice. A reader looks up the keys it
// knows and passes over the rest, the way a process reads its ELF auxiliary vector, so
// a toc that grows an entry is still read by a host that predates it.

import type { u32 } from "@xpute/kit/abi/word.ts";

/** "XPUT" as four bytes read little-endian: what tells a toc from memory nothing wrote. */
export const MAGIC: u32 = 1414877272 as number;

/** Raised when a key comes to mean something else. A new key does not raise it. */
export const VERSION: u32 = 1 as number;

/** Words before the first entry. */
export const HEAD: u32 = 4 as number;

/** What an entry's value is. 3 to 5 are retired and given to nothing else: they were the
 * numbers of a host's scheduling policy, which the program now injects when the host
 * boots (the host's sched/policy.ts). */
export const key = {
  /** Where the submission ring's words begin. */
  SQ: 1 as number,
  /** Where the completion ring's words begin. */
  CQ: 2 as number,
  /** Where the hostcall stream's words begin (ipc/sys.json). */
  SYS: 6 as number,
  /** The bit every key a program numbers for itself carries: xpute gives none of
   * them a meaning, and a host that is not the program's passes over them. */
  PROGRAM: 2147483648 as number,
} as const;
