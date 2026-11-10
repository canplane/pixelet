// @xpute/runtime/conformance.spec.ts
//
// GENERATED from spec/conformance/core.json
//
// The conformance core, which the contract's traces drive (spec/conformance/door.tsv):
// the few commands that every language's conformance core implements, and the numbers a
// trace is written against. The core computes nothing, and asks for no turn when nothing
// waits. How it works inside is each core's own; the traces hold only what the runtime
// sees of it.
//
// A trace names no number a production core uses. These numbers are the fixture's own,
// chosen so that a trace can make a turn's output outgrow its room without knowing how a
// core reserves that room.

import type { u32 } from "@xpute/kit/abi/word.ts";
import * as cmd from "./conformance/cmd.spec.ts";

/** The size of the range the core shares with its runtime. The range holds the front's
 * header, both rings, the stream and the gaps LAYOUTS draws between them, then the core's
 * own state and heap. The core's boot answers where the range begins, and so where the
 * header is (the contract's rule 0). The rest of the memory, a module's stack and data,
 * is the build's, and no number here sizes it. */
export const MEM_BYTES: u32 = 262144 as number;

/** How many layouts a runtime runs every trace against: one for each seed below this
 * number. From the seed, the core's boot draws with kit's SplitMix32 the order of the
 * parts it shares with its runtime, the header among them, and how many pages to leave
 * before each part. Every core draws the same way. No shared part lies where it lay under
 * another seed, so only a runtime that finds every part through the header passes every
 * layout (the contract's rule 0). No machine fixes where these parts lie, so the fixture
 * lays its front and writes its header itself, as a runtime does for a machine. */
export const LAYOUTS: u32 = 8 as number;

/** The most pages a layout leaves before each part it lays. */
export const GAP_PAGES: u32 = 3 as number;

/** How many frames the submission ring holds. */
export const SQ_CAP: u32 = 16 as number;

/** How many frames the completion ring holds. It is less than what eight commands leave
 * when each raises MOST_RAISE frames, so a trace that sends eight of them makes a turn's
 * output outgrow the ring. */
export const CQ_CAP: u32 = 128 as number;

/** The size of one ring slot in bytes, enough for a packet of one number. */
export const SLOT_BYTES: u32 = 256 as number;

/** The length in words of the stream's range, its END word first. The header's STREAM_AT word
 * points at the stream, so the runtime runs it after every turn as it runs a GPU stream. */
export const STREAM_WORDS: u32 = 256 as number;

/** The size of the stack a module core is linked with, which comes first in its memory
 * (rust/dev/conformance/build.rs, cpp/Makefile). */
export const STACK_BYTES: u32 = 1048576 as number;

/** The size of the core's heap, which lies in a range of its own. The memory a runtime hands
 * a module never grows, and a core takes no memory it was not given. */
export const HEAP_BYTES: u32 = 65536 as number;

/** The core's handle table has this many slots, and slot 0 is never handed out. */
export const HANDLES: u32 = 16 as number;

/** The most frames one RAISE may ask for; a RAISE that asks for more is refused with EINVAL. */
export const MOST_RAISE: u32 = 32 as number;

/** The conformance core's own commands, numbered `major << 8 | minor` past xpute's majors
 * (ipc/frame.json). Where a command takes a packet, the packet is a branch whose first
 * child is a number. */
export { cmd };
