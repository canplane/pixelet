// @pxl/client/port/surface.ts

/** SURFACE and TURN on the shell's port: the surfaces the shell lends the
 * kernel, and the grant each turn the strobe grants submits before it rings. */

import type { u32 } from "@xpute/kit/abi/word.ts";
import type { FrameGrant } from "@xpute/host";
import { type SurfaceDriver } from "@/services/types.ts";
import * as origin from "@/services/origin.ts";
import { kernel } from "@/host/kernel.ts";

/** A surface as the place drives it: 0 opened when the kernel has none left;
 * `draw`, where given, says what it draws as it opens (scene.ts). */
export function surface_driver(draw?: (surface: u32) => void): SurfaceDriver {
  return {
    open: () => {
      const surface = Math.max(0, kernel.surface_open());
      if (surface !== 0) draw?.(surface);
      return surface;
    },
    size: (surface, width, height, dpr) =>
      kernel.surface_resize(surface, width, height, dpr),
    close: (surface) => void kernel.surface_close(surface),
  };
}

/** The turn's grant, told the kernel before each turn the strobe grants:
 * its pacing, whether it is interactive, the reads allowed out and the
 * clock. The strobe runs only while the world's view is open, so every turn
 * it grants is one the view draws. */
export function submit_grant(grant: FrameGrant): void {
  kernel.turn_grant(
    grant.refresh_ms,
    grant.paced_ms,
    grant.interactive,
    origin.credits,
    Date.now(),
  );
}
