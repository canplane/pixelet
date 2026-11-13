// @pxl/client/service/core/surface.ts

/** The surfaces the UI lends the GPU (xpute gpu/wit/surface.wit): each an
 * object the core serves, opened over the UI's remote end of the link, its
 * canvas given beside the link under its handle, and dropped to close it,
 * which takes the canvas back. */

import type { u32 } from "@xpute/kit/abi/word.ts";
import * as surface from "@xpute/gpu-link/abi/surface.ts";
import { drop, open } from "@xpute/link";
import { type SurfaceDriver } from "@/service/platform/types.ts";
import { ui } from "@/service/core/core.ts";

export function surface_driver(draw?: (surface: u32) => void): SurfaceDriver {
  return {
    open: async () => {
      // A surface refused is none.
      const h = await open(ui, surface.NAME).catch(() => 0);
      if (h !== 0) draw?.(h);
      return h;
    },
    size: (h, width, height, dpr) =>
      void surface.surface_resize({ link: ui, handle: h }, width, height, dpr),
    close: (h) => drop(ui, h),
    bind: (h, canvas) => ui.give(h, canvas.transferControlToOffscreen()),
  };
}
