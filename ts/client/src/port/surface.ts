// @pxl/client/port/surface.ts

/** The surfaces the shell lends the kernel, on xpute's GPU device
 * (GPU_SURFACE_*), and TURN on the shell's port: the grant each turn the
 * scheduler grants submits before it rings. */

import type { u32 } from "@xpute/kit/abi/word.ts";
import { type FrameGrant, gpu_exports } from "@xpute/runtime";
import { type SurfaceDriver } from "@/service/types.ts";
import * as origin from "@/service/origin.ts";
import { kernel, ui } from "@/runtime/kernel.ts";

/** A surface as the place drives it: 0 opened when the kernel has none left;
 * `draw`, where given, says what it draws as it opens (scene.ts). */
export function surface_driver(draw?: (surface: u32) => void): SurfaceDriver {
  return {
    open: async () => {
      const surface = Math.max(0, await gpu_exports(ui).gpu_surface_open());
      if (surface !== 0) draw?.(surface);
      return surface;
    },
    size: (surface, width, height, dpr) =>
      gpu_exports(ui).gpu_surface_resize(surface, width, height, dpr),
    close: (surface) => gpu_exports(ui).gpu_surface_close(surface),
    bind: (surface, canvas) => ui.bind(surface, canvas),
    unbind: (surface) => ui.unbind(surface),
  };
}

/** The turn's grant, told the kernel before each turn the scheduler grants,
 * ahead of the runtime's SYS_TURN, which carries the clock: its pacing,
 * whether it is interactive and the reads allowed out. The scheduler runs only
 * while the world's view is open, so every turn it grants is one the view
 * draws. */
export function submit_grant(grant: FrameGrant): void {
  kernel.turn_grant(
    grant.refresh_ms,
    grant.paced_ms,
    grant.interactive,
    origin.credits,
  );
}
