// @pxl/client/port/surface.ts

/** SURFACE on the shell's port: the surfaces the shell lends the kernel, and
 * what each turn the strobe grants submits before it rings. */

import type { f64, u32 } from "@xpute/kit/abi/word.ts";
import type { FrameGrant } from "@xpute/host";
import { bracket, type SurfaceDriver } from "@/bracket.tsx";
import {
  Command,
  encode_surface_close,
  encode_surface_frame,
  encode_surface_open,
  encode_surface_resize,
} from "@/abi/cmd.spec.ts";
import { HOST } from "@/host/mount.ts";
import * as port from "@/abi/port.spec.ts";

/** 0 when the kernel has none left. */
function surface_open(role: u32): u32 {
  return HOST.call(
    port.SHELL,
    Command.SURFACE_OPEN,
    encode_surface_open(HOST.doorbell, role),
    (result) => Math.max(0, result),
  );
}

function surface_size(surface: u32, width: u32, height: u32, dpr: f64): void {
  HOST.send(
    port.SHELL,
    Command.SURFACE_RESIZE,
    encode_surface_resize(HOST.doorbell, surface, width, height, dpr),
  );
}

function surface_close(surface: u32): void {
  HOST.call(
    port.SHELL,
    Command.SURFACE_CLOSE,
    encode_surface_close(HOST.doorbell, surface),
    (result) => result,
  );
}

/** A surface of `role` (gpu.spec.ts `surface_role`) as the place drives it. */
export function surface_driver(role: u32): SurfaceDriver {
  return {
    open: () => surface_open(role),
    size: surface_size,
    close: surface_close,
  };
}

/** The turn's frame, the host's alone: the turn's pacing, whether it is
 * interactive, the reads allowed out and the clock. The view it is drawn at
 * is the world surface's (SURFACE_RESIZE). The strobe runs only while the
 * world's view is open, so every turn it grants is one the view draws. */
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
