// @pxl/client/service/core/turn.ts

/** TURN on the UI's port: before each turn the scheduler grants is rung, its
 * grant is submitted here. */

import type { FrameGrant } from "@xpute/runtime";
import * as origin from "@/service/platform/origin.ts";
import { core } from "@/service/core/core.ts";

/** Tells the core the grant of a turn the scheduler is about to run: its
 * pacing, whether it is interactive, and how many reads may go out. It goes
 * ahead of the runtime's SYS_TURN, which carries the clock. The scheduler runs
 * only while the world's view is open, so every turn it grants is one the view
 * draws. */
export function submit_grant(grant: FrameGrant): void {
  core.turn_grant(
    grant.refresh_ms,
    grant.paced_ms,
    grant.interactive,
    origin.credits,
  );
}
