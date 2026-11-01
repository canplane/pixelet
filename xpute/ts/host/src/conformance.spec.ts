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
import * as key from "./conformance/key.spec.ts";
import * as hostcall from "./conformance/hostcall.spec.ts";
import * as cmd from "./conformance/cmd.spec.ts";

/** The room a guest's shared range asks for: the toc, both rings, the two streams
 * and the gaps LAYOUTS draws between them, then its own state and heap. Where the range
 * begins, and so where the toc is, the guest's boot answers (the contract's rule 0);
 * what else a memory holds — a module's stack and data — is the build's, and no number here. */
export const MEM_BYTES: u32 = 262144 as number;

/** The layouts a host runs every trace against, one for each seed below this. The guest's
 * boot takes the seed and draws from it (kit's SplitMix32) the order of what it shares
 * with its host, the toc among them, and the pages left before each, every guest by
 * the same draw; so nothing it shares lies where it lay under another seed, and only a
 * host that finds it all through the toc passes every layout (the contract's rule 0). */
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

/** The guest's own entry in its toc, past xpute's PROGRAM bit. */
export { key };

/** The guest's own hostcall, past xpute's PROGRAM bit. */
export { hostcall };

/** The port a trace sends the guest's own commands as, and the one the guest raises
 * its events for (ipc/frame.json PORT_SHIFT). */
export const SENDER: u32 = 1 as number;

/** The guest's own commands, `major << 8 | minor`, which a trace sends as SENDER. A packet,
 * where one is taken, is a branch whose first child is a number. */
export { cmd };
