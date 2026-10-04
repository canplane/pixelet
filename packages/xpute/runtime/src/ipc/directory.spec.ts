// @xpute/runtime/ipc/directory.spec.ts
//
// GENERATED from spec/xpute/ipc/directory.json — do not edit.
//
// The directory: where a host finds what its guest laid out in the memory. The guest's
// first turn writes it, and the host reads it once, at the one offset it knows outside
// the memory (xpute-runtime ipc/directory.rs, and the contract's rule 0 in
// ARCHITECTURE.xpute.md).
//
// Its words: MAGIC, VERSION, how many entries follow, and a word kept at 0; then that
// many entries, a key and a value each, no key twice. A reader looks up the keys it
// knows and passes over the rest, the way a process reads its ELF auxiliary vector, so
// a directory that grows an entry is still read by a host that predates it.

import type { u32 } from "@xpute/core/abi/word.ts";

/** "XPUT" as four bytes read little-endian: what tells a directory from memory nothing wrote. */
export const MAGIC: u32 = 1414877272 as number;

/** Raised when a key comes to mean something else. A new key does not raise it. */
export const VERSION: u32 = 1 as number;

/** Words before the first entry. */
export const HEAD: u32 = 4 as number;

/** What a quantum policy's number is multiplied by to cross as a word: it crosses in thousandths. */
export const QUANTUM_SCALE: u32 = 1000 as number;

/** What an entry's value is. */
export const key = {
  /** Where the submission ring's words begin. */
  SUBMISSION: 1 as number,
  /** Where the completion ring's words begin. */
  COMPLETION: 2 as number,
  /** The quantum policy's margin share (sched/quantum.rs), in QUANTUM_SCALE. */
  MARGIN_SHARE: 3 as number,
  /** The quantum policy's batch frames, in QUANTUM_SCALE. */
  BATCH_FRAMES: 4 as number,
  /** The quantum policy's settle time, in QUANTUM_SCALE of a millisecond. */
  SETTLE_MS: 5 as number,
  /** Where the system-call stream's words begin (ipc/sys.json). */
  SYS: 6 as number,
  /** The bit every key a program numbers for itself carries: xpute gives none of
   * them a meaning, and a host that is not the program's passes over them. */
  PROGRAM: 2147483648 as number,
} as const;
