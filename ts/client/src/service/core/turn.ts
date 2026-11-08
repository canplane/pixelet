// @pxl/client/service/core/turn.ts

import type { FrameGrant } from "@xpute/runtime";
import * as origin from "@/service/platform/origin.ts";
import { core } from "@/service/core/core.ts";

/** The grant goes ahead of the runtime's SYS_TURN, which carries the clock.
 * The scheduler runs only while the world's view is open, so every turn it
 * grants is one the view draws. */
export function submit_grant(grant: FrameGrant): void {
  core.turn_grant(
    grant.refresh_ms,
    grant.paced_ms,
    grant.interactive,
    origin.credits,
  );
}
