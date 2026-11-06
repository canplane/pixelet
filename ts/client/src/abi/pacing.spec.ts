// @pxl/client/abi/pacing.spec.ts
//
// GENERATED from spec/client/kernel/sys/sched/pacing.json
//
// The numbers of the scheduling policy pixelet boots xpute's runtime with on a page or a
// device (@xpute/runtime sched/frame_pacing.ts): turns paced to the display, each given
// what its frames leave. The client's boot injects them (ts/client/src/abi/boot.ts),
// and the kernel reads the margin to size its tasks past a full frame
// (pxl-kernel sys/sched/edge.rs).

import type { f64 } from "@xpute/kit/abi/word.ts";

/** The slowest rate turns are paced at: the most whole refreshes of the display that fit
 * one interval at this rate, one at least, so a 120 Hz display runs a turn every second
 * refresh. */
export const PACE_HZ: f64 = 60 as number;

/** Share of the frame left unspent for the runtime's own work; a share, so a fast display
 * does not lose most of its frame to it. */
export const MARGIN_SHARE: f64 = 0.15 as number;

/** Frames a non-interactive turn may take: with nothing moving, two vsyncs only show the same image twice. */
export const BATCH_FRAMES: f64 = 2 as number;

/** How long after the last input a turn is still interactive, so a pause mid-drag does not count. */
export const SETTLE_MS: f64 = 150 as number;
