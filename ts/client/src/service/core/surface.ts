// @pxl/client/service/core/surface.ts

/** The surfaces the UI lends the GPU (xpute wit/gpu/surface.wit): each an
 * object the core serves, opened over the UI's remote end of the link, its
 * canvas given beside the link under its handle, and released to close it,
 * which takes the canvas back. */

import type { u32 } from "@xpute/kit/abi/word.ts";
import * as surface from "@xpute/gpu/abi/surface.ts";
import { open, release, uri } from "@xpute/runtime";
import { type SurfaceDriver } from "@/service/platform/types.ts";
import { ui } from "@/service/core/core.ts";

const SURFACE = uri("xpute:gpu", "surface");

export function surface_driver(draw?: (surface: u32) => void): SurfaceDriver {
  return {
    open: async () => {
      // A surface refused is none.
      const h = await open(ui, SURFACE).catch(() => 0);
      if (h !== 0) draw?.(h);
      return h;
    },
    size: (h, width, height, dpr) =>
      void surface.surface_resize({ link: ui, handle: h }, width, height, dpr),
    close: (h) => release(ui, h),
    bind: (h, canvas) => ui.give(h, canvas.transferControlToOffscreen()),
  };
}
