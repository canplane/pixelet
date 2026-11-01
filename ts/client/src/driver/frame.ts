// @pxl/client/driver/frame.ts

/** What each turn the strobe grants submits before it rings: the host's alone,
 * the turn's pacing, whether it is interactive, the reads allowed out and the
 * clock. The view it is drawn at is the world surface's (SURFACE_RESIZE).
 * The strobe runs only while the world's view is open, so every turn it grants
 * is one the view draws. */

import type { FrameGrant } from "@xpute/host/sched/frame_pacing.ts";
import { Command, encode_surface_frame } from "@/abi/cmd.spec.ts";
import { bracket } from "@/bracket.tsx";
import { HOST } from "@/host/mount.ts";
import * as port from "@/abi/port.spec.ts";

export function submit_frame(grant: FrameGrant): void {
  HOST.doorbell.send(
    port.SHELL,
    Command.SURFACE_FRAME,
    encode_surface_frame(
      HOST.doorbell,
      grant.refresh_ms,
      grant.paced_ms,
      grant.interactive,
      bracket.origin.credits,
      Date.now(),
    ),
  );
}
