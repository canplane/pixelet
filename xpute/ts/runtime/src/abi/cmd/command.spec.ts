// @xpute/runtime/abi/cmd/command.spec.ts
//
// GENERATED from spec/abi/cmd.json

// Every command, by the side that sends it.

// ---- runtime, export ----

/** Replies with its packet's value, or 0 where it has none — the rings' own test. */
export const NOP = 0x0000;
/** A runtime has attached, the first or again after one before it went: the mount sends it
 * once the front is laid. What the old runtime's requests leave behind is the core's to let go. */
export const SYS_ATTACH = 0x0107;
/** The core is about to be frozen, its turns stopped for as long as its runtime is hidden:
 * its last sure moment to save. A call, rung at once. */
export const SYS_SUSPEND = 0x0108;
/** The core woken from SYS_SUSPEND. A call, rung at once. */
export const SYS_RESUME = 0x0109;
/** The GPU the machine asked for answered, the first time or again after a loss, whose
 * sign is in the stream's device block. */
export const GPU_READY = 0x0201;

// ---- runtime, import ----

/** The broken invariant the core reports as it stops (xpute-kit status/bug.rs). */
export const SYS_ABORT = 0x0106;
