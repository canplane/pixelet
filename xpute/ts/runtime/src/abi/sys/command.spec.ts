// @xpute/runtime/abi/sys/command.spec.ts
//
// GENERATED from spec/abi/sys.json

// Every command, by the side that sends it.

// ---- sys, export ----

/** Replies with its packet's value, or 0 where it has none — the rings' own test. */
export const NOP = 0x0000;
/** A runtime has attached, the first or again after one before it went: the mount sends it
 * once the front is laid. What the old runtime's requests leave behind is the core's to let go. */
export const SYS_ATTACH = 0x0102;
/** The core is about to be frozen, its turns stopped for as long as its runtime is hidden:
 * its last sure moment to save. A call, rung at once. */
export const SYS_SUSPEND = 0x0103;
/** The core woken from SYS_SUSPEND. A call, rung at once. */
export const SYS_RESUME = 0x0104;
/** A turn the scheduling policy grants, sent before it rings after what the program
 * submits for it, as against a turn rung for a command alone: the wall clock, in Unix
 * milliseconds. What else the policy says of the turn is the program's to send. */
export const SYS_TURN = 0x0105;

// ---- sys, import ----

/** The broken invariant the core reports as it stops (xpute-kit status/bug.rs). */
export const SYS_ABORT = 0x0101;
