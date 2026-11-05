// @xpute/host/abi/cmd/command.spec.ts
//
// GENERATED from spec/abi/cmd.json

// Every command, by the side that sends it.

// ---- host, export ----

/** Replies with its packet's value, or 0 where it has none — the rings' own test. */
export const NOP = 0x0000;
/** A host has attached, the first or again after one before it went: the mount sends it
 * once the front is laid. What the old host's requests leave behind is the guest's to let go. */
export const SYS_ATTACH = 0x0107;
/** The guest is about to be frozen, its turns stopped for as long as its host is hidden:
 * its last sure moment to save. A call, rung at once. */
export const SYS_SUSPEND = 0x0108;
/** The guest woken from SYS_SUSPEND. A call, rung at once. */
export const SYS_RESUME = 0x0109;
/** The GPU the machine asked for answered, the first time or again after a loss, whose
 * sign is in the stream's device block. */
export const GPU_READY = 0x0201;

// ---- host, import ----

/** The broken invariant the guest reports as it stops (xpute-kit status/bug.rs). */
export const SYS_ABORT = 0x0106;
