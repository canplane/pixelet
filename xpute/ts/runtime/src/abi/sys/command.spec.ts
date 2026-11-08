// @xpute/runtime/abi/sys/command.spec.ts
//
// GENERATED from spec/abi/sys.json

// Every command, by the side that sends it.

// ---- sys, export ----

/** The rings' own test. The reply carries the packet's value, or 0 when the frame has no
 * packet. */
export const NOP = 0x0000;
/** A runtime has attached, for the first time or after an earlier runtime went away. The
 * runtime sends it once it has laid the front of the core's memory (ipc/head.json). The core
 * lets go of whatever the earlier runtime's requests left behind. */
export const SYS_ATTACH = 0x0102;
/** The core is about to be frozen: its turns stop for as long as its runtime is hidden. This
 * is the core's last sure moment to save. It is a call, and the runtime rings the core at
 * once to have it answered. */
export const SYS_SUSPEND = 0x0103;
/** The core has been woken after SYS_SUSPEND. It is a call, rung at once as SYS_SUSPEND is. */
export const SYS_RESUME = 0x0104;
/** Marks a turn that the scheduling policy grants, as opposed to a turn rung only because a
 * command arrived. The runtime submits it after what the program submits for that turn,
 * and before it rings. It carries the wall clock in Unix milliseconds. Anything else the
 * policy has to say about the turn, the program sends in commands of its own. */
export const SYS_TURN = 0x0105;

// ---- sys, import ----

/** Reports the broken invariant the core stops on (xpute-kit status/bug.rs). */
export const SYS_ABORT = 0x0101;
