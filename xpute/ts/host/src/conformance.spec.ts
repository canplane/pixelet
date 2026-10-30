// @xpute/host/conformance.spec.ts
//
// GENERATED from spec/conformance/guest.json
//
// The guest the contract's traces drive (spec/conformance/door.tsv): the
// few commands every language's conformance guest implements, and the numbers a trace
// is written against. It computes nothing, and asks for no turn when nothing waits; what
// it does inside is each guest's own, and what the host sees of it is what the traces
// hold it to.
//
// A trace names no production number. These are the fixture's, chosen so that a trace
// can make a turn's output outgrow its room without knowing how a guest reserves it.

import type { u32 } from "@xpute/kit/abi/word.ts";

/** The room a guest's shared range asks for: the directory, both rings, the two streams
 * and the gaps LAYOUTS draws between them, then its own state and heap. Where the range
 * begins, and so where the directory is, the guest's boot answers (the contract's rule 0);
 * what else a memory holds — a module's stack and data — is the build's, and no number here. */
export const MEM_BYTES: u32 = 262144 as number;

/** The layouts a host runs every trace against, one for each seed below this. The guest's
 * boot takes the seed and draws from it (kit's SplitMix32) the order of what it shares
 * with its host, the directory among them, and the pages left before each, every guest by
 * the same draw; so nothing it shares lies where it lay under another seed, and only a
 * host that finds it all through the directory passes every layout (the contract's rule 0). */
export const LAYOUTS: u32 = 8 as number;

/** The most pages a layout leaves before each part it lays. */
export const GAP_PAGES: u32 = 3 as number;

/** Frames the submission ring holds. */
export const SQ_CAP: u32 = 16 as number;

/** Frames the completion ring holds: fewer than eight commands raising MOST_RAISE
 * events each leave, so a trace that sends those makes a turn's output outgrow it. */
export const CQ_CAP: u32 = 128 as number;

/** A ring slot's payload: a packet of one number fits. */
export const SLOT_BYTES: u32 = 256 as number;

/** The stream's range, its END word first. */
export const STREAM_WORDS: u32 = 256 as number;

/** The hostcalls' stream: OPEN and CLOSE are recorded there, and nothing runs them. */
export const SYS_WORDS: u32 = 64 as number;

/** The stack a module guest is linked with, first in its memory (rust/conformance/build.rs, cpp/Makefile). */
export const STACK_BYTES: u32 = 1048576 as number;

/** The guest's heap, in its own range, since the memory a host hands a module never grows
 * and a guest takes nothing it was not given. */
export const HEAP_BYTES: u32 = 65536 as number;

/** Slots in the guest's handle table, slot 0 never handed out. */
export const HANDLES: u32 = 16 as number;

/** The most events one RAISE asks for; more is refused with EINVAL. */
export const MOST_RAISE: u32 = 32 as number;

/** The guest's own entry in its directory, past xpute's PROGRAM bit. */
export const key = {
  /** Where the stream is, run by the host after every turn. */
  STREAM: 2147483649 as number,
} as const;

/** The guest's own hostcall, past xpute's PROGRAM bit. */
export const hostcall = {
  /** A number and an offset: the host writes the number plus one at the offset and answers 0,
   * before the hostcall returns; a number of 0 it refuses with EINVAL, writing nothing. */
  ANSWER: 2147483649 as number,
} as const;

/** The port a trace sends the guest's own commands as, and the one the guest raises
 * its events for (ipc/frame.json PORT_SHIFT). */
export const SENDER: u32 = 1 as number;

/** The guest's own commands, `major << 8 | minor`, which a trace sends as SENDER. A packet,
 * where one is taken, is a branch whose first child is a number. */
export const cmd = {
  /** Succeeds; replies 0. */
  OK: 4097 as number,
  /** Fails with EINVAL. */
  FAIL: 4098 as number,
  /** Raises the packet's number of EVENTs, tagged 0 up, then replies with that number. */
  RAISE: 4099 as number,
  /** Writes one record to the stream, its op the packet's number and no words; replies 0. */
  RECORD: 4100 as number,
  /** Takes a handle from the guest's table and replies with it. */
  ISSUE: 4101 as number,
  /** Gives back the packet's handle; fails with EBADF for one that is not live. */
  RELEASE: 4102 as number,
  /** Replies 0 for a live handle; fails with EBADF for any other, 0 among them. */
  USE: 4103 as number,
  /** Reads the host's clock and keeps the reading; replies 0. */
  MARK: 4104 as number,
  /** Reads the host's clock and replies with the whole milliseconds since the last MARK. */
  SINCE: 4105 as number,
  /** Opens a file for the packet's number, its owner, and replies with the descriptor; the host's answer (SYS_OPENED) raises OPENED for that owner while the descriptor is open. */
  OPEN: 4106 as number,
  /** Closes the packet's descriptor, answered or not; fails with EBADF for one not open. */
  CLOSE: 4107 as number,
  /** Zeroes a word of its own, makes the hostcall ANSWER of the packet's number and the word's offset, and replies with the word as it then reads it; fails with the host's errno where the host refused. */
  HOSTCALL: 4108 as number,
  /** What RAISE raises. */
  EVENT: 4353 as number,
  /** An open answered: tagged with its owner, its result the host's. */
  OPENED: 4354 as number,
} as const;
