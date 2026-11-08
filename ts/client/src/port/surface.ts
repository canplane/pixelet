// @pxl/client/port/surface.ts

/** The surfaces the UI lends the core, on xpute's GPU device
 * (GPU_SURFACE_*), and TURN on the UI's port: before each turn the scheduler
 * grants is rung, its grant is submitted here. */

import type { u32 } from "@xpute/kit/abi/word.ts";
import { type FrameGrant, gpu_exports } from "@xpute/runtime";
import { type SurfaceDriver } from "@/service/types.ts";
import * as origin from "@/service/origin.ts";
import { core, ui } from "@/port/core.ts";

/** A surface as the place drives it. Opening answers 0 when the core has no
 * surface left. `draw`, when given, says what the surface draws as soon as it
 * opens (scene.ts). */
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
