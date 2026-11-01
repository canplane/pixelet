// @xpute/host/program.spec.ts
//
// GENERATED from spec/program.json
//
// A program's defaults (xpute-guest program.rs `program!`, cpp/xpute/guest/program.hpp
// `XPUTE_PROGRAM`): the port its host speaks as, what its range holds and the stack it
// is linked with, each where the program says nothing else; and the files it may have
// open. The range's offsets follow from the counts, as each guest end computes them.

import type { u32 } from "@xpute/kit/abi/word.ts";

/** The port a program's host speaks as unless it says otherwise, and the one a handler's
 * events are raised for: any but the host's, 0 (ipc/frame.json HOST). */
export const PORT: u32 = 1 as number;

/** Each ring's slots: how many commands can wait, and replies and events be left,
 * between two turns. */
export const CAP: u32 = 64 as number;

/** A slot's bytes: the most a command's packet, or an event's, may be. */
export const SLOT_BYTES: u32 = 1024 as number;

/** The hostcalls' stream, in words: what one turn may ask the host to run after it. */
export const SYS_WORDS: u32 = 4096 as number;

/** The events one command may raise beside its reply. */
export const MOST_EVENTS: u32 = 4 as number;

/** The stack a module is linked with (`-z stack-size`), placed first. */
export const STACK_BYTES: u32 = 1048576 as number;

/** The slots of the table its open files are held in, each a read awaited: one fewer may
 * be open at once, and a read past them is refused with EMFILE. */
export const MOST_FILES: u32 = 64 as number;
