// @pxl/client/driver/frame.ts

/** What each turn the strobe grants submits before it rings: the host's alone,
 * the turn's pacing, whether it is interactive, the reads allowed out and the
 * clock. The view it is drawn at is the shell's (port/input.ts `kernel_rect`).
 * The strobe runs only while the world's view is open, so every turn it grants
 * is one the view draws. */

import type { FrameGrant } from "@xpute/host/sched/frame_pacing.ts";
import { Command, encode_world_frame } from "@/abi/cmd.spec.ts";
import { bracket } from "@/bracket.tsx";
import { HOST } from "@/host/mount.ts";

export function submit_frame(grant: FrameGrant): void {
  HOST.doorbell.send(
    0,
    Command.WORLD_FRAME,
    encode_world_frame(
      HOST.doorbell,
      grant.refresh_ms,
      grant.paced_ms,
      grant.interactive,
      bracket.origin.credits,
      Date.now(),
    ),
  );
}
