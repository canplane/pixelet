// @pxl/client/port/surface.ts

/** SURFACE and TURN on the shell's port: the surfaces the shell lends the
 * kernel, and the grant each turn the scheduler grants submits before it rings. */

import type { u32 } from "@xpute/kit/abi/word.ts";
import type { FrameGrant } from "@xpute/runtime";
import { type SurfaceDriver } from "@/service/types.ts";
import * as origin from "@/service/origin.ts";
import { control, kernel } from "@/runtime/kernel.ts";

/** A surface as the place drives it: 0 opened when the kernel has none left;
 * `draw`, where given, says what it draws as it opens (scene.ts). */
export function surface_driver(draw?: (surface: u32) => void): SurfaceDriver {
  return {
    open: async () => {
      const surface = Math.max(0, await kernel.surface_open());
      if (surface !== 0) draw?.(surface);
      return surface;
    },
    size: (surface, width, height, dpr) =>
      kernel.surface_resize(surface, width, height, dpr),
    close: (surface) => kernel.surface_close(surface),
    bind: (surface, canvas) => control.bind(surface, canvas),
    unbind: (surface) => control.unbind(surface),
  };
}

/** The turn's grant, told the kernel before each turn the scheduler grants:
 * its pacing, whether it is interactive, the reads allowed out and the
 * clock. The scheduler runs only while the world's view is open, so every turn
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
