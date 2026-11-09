// @pxl/client/service/core/turn.ts

import type { FrameGrant } from "@xpute/runtime";
import { Command } from "@/abi/cmd.spec.ts";
import { core } from "@/service/core/core.ts";

/** The grant goes ahead of the runtime's SYS_TURN, which carries the clock.
 * The scheduler runs only while the world's view is open, so every turn it
 * grants is one the view draws. */
export function submit_grant(grant: FrameGrant): void {
  core.send(
    Command.TURN_GRANT,
    grant.refresh_ms,
    grant.paced_ms,
    grant.interactive,
  );
}
