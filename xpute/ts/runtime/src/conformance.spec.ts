// @xpute/runtime/conformance.spec.ts
//
// GENERATED from spec/conformance/core.json
//
// The core the contract's traces drive (spec/conformance/door.tsv): the
// few commands every language's conformance core implements, and the numbers a trace
// is written against. It computes nothing, and asks for no turn when nothing waits; what
// it does inside is each core's own, and what the runtime sees of it is what the traces
// hold it to.
//
// A trace names no production number. These are the fixture's, chosen so that a trace
// can make a turn's output outgrow its room without knowing how a core reserves it.

import type { u32 } from "@xpute/kit/abi/word.ts";
import * as cmd from "./conformance/cmd.spec.ts";

/** The room a core's shared range asks for: the front's header, both rings, the stream
 * and the gaps LAYOUTS draws between them, then its own state and heap. Where the range
 * begins, and so where the header is, the core's boot answers (the contract's rule 0);
 * what else a memory holds — a module's stack and data — is the build's, and no number here. */
export const MEM_BYTES: u32 = 262144 as number;

/** The layouts a runtime runs every trace against, one for each seed below this. The core's
 * boot takes the seed and draws from it (kit's SplitMix32) the order of what it shares
 * with its runtime, the header among them, and the pages left before each, every core by
 * the same draw; so nothing it shares lies where it lay under another seed, and only a
 * runtime that finds it all through the header passes every layout (the contract's rule 0).
 * The fixture lays its front and writes its header itself, as a runtime does for a machine,
 * since no machine fixes where its parts lie. */
export const LAYOUTS: u32 = 8 as number;

/** The most pages a layout leaves before each part it lays. */
export const GAP_PAGES: u32 = 3 as number;

/** Frames the submission ring holds. */
export const SQ_CAP: u32 = 16 as number;

/** Frames the completion ring holds: fewer than eight commands raising MOST_RAISE
 * frames each leave, so a trace that sends those makes a turn's output outgrow it. */
export const CQ_CAP: u32 = 128 as number;

/** A ring slot's payload: a packet of one number fits. */
export const SLOT_BYTES: u32 = 256 as number;

/** The stream's range, its END word first. The header's GPU_AT names it: the runtime runs
 * it after every turn, as it runs a GPU stream. */
export const STREAM_WORDS: u32 = 256 as number;

/** The stack a module core is linked with, first in its memory (rust/conformance/build.rs, cpp/Makefile). */
export const STACK_BYTES: u32 = 1048576 as number;

/** The core's heap, in its own range, since the memory a runtime hands a module never grows
 * and a core takes nothing it was not given. */
export const HEAP_BYTES: u32 = 65536 as number;

/** Slots in the core's handle table, slot 0 never handed out. */
export const HANDLES: u32 = 16 as number;

/** The most frames one RAISE asks for; more is refused with EINVAL. */
export const MOST_RAISE: u32 = 32 as number;

/** The core's own commands, `major << 8 | minor`, past xpute's majors (ipc/frame.json). A
 * packet, where one is taken, is a branch whose first child is a number. */
export { cmd };
