// @xpute/runtime/abi/sys/command.spec.ts
//
// GENERATED from spec/abi/sys.json

// Every command, by the side that sends it.

// ---- sys, export ----

/** The channel's own test. The reply carries the packet's value, or 0 when the frame has no
 * packet. */
export const NOP = 0x0000;
/** A runtime has attached, for the first time or after an earlier runtime went away. The
 * runtime sends it once it has laid the front of the core's memory (io/head.json). The core
 * lets go of whatever the earlier runtime's requests left behind. */
export const SYS_ATTACH = 0x0102;
/** The core is about to be frozen: its turns stop for as long as its runtime is hidden. This
 * is the core's last sure moment to save. The UI sends it and does not wait; the runtime
 * lets it reach the core in a turn before turns stop. */
export const SYS_SUSPEND = 0x0103;
/** The core has been woken after SYS_SUSPEND. */
export const SYS_RESUME = 0x0104;
/** Marks a turn that the scheduling policy grants, as opposed to a turn run only because a
 * command arrived. The runtime submits it after what the program submits for that turn,
 * and before it runs the turn. It carries the wall clock in Unix milliseconds. What frame pacing
 * says of the turn comes before it (SYS_FRAME); anything else the policy has to say, the
 * program sends in commands of its own. */
export const SYS_TURN = 0x0105;
/** The display frame frame pacing grants the next SYS_TURN at (@xpute/runtime
 * sched/frame_pacing.ts, xpute-runtime native/pacing.rs): the display's refresh, the interval
 * turns are paced at, a whole number of refreshes and the frame's budget, and whether the
 * turn follows an input. A runtime whose policy paces no display sends none. */
export const SYS_FRAME = 0x0106;

// ---- sys, import ----

/** Reports the broken invariant the core stops on (xpute-kit status/bug.rs). */
export const SYS_ABORT = 0x0101;
/** The least interval the core asks between its turns, 0 for none: its load's word on the
 * rate, which the next SYS_FRAME's `paced_ms` answers, a whole number of refreshes. A policy
 * that paces no display ignores it. */
export const SYS_PACE = 0x0107;
