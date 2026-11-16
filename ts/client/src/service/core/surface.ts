// @pxl/client/service/core/surface.ts

/** The surfaces the UI lends the GPU (spec/gpu/wit/surface.wit): each an
 * object the core implements, opened over the UI's end of the link, a page's
 * canvas given beside the link under its handle (service/platform
 * surface.web.tsx), and dropped to close it, which takes the canvas back. */

import type { u32 } from "@xpute/kit/abi/word.ts";
import * as surface from "@pxl/gpu/host/abi/surface.ts";
import { SURFACE_URI } from "@pxl/gpu/link/api.spec.ts";
import { drop, open, uri } from "@xpute/link";
import { type SurfaceDriver } from "@/service/platform/types.ts";
import { ui } from "@/service/core/core.ts";

/** A surface's driver; `draw` runs once it opens, and `closed` once it is
 * dropped. */
export function surface_driver(
  draw?: (surface: u32) => void,
  closed?: (surface: u32) => void,
): SurfaceDriver {
  return {
    open: async () => {
      // A surface refused is none.
      const h = await open(ui, uri(...SURFACE_URI)).catch(() => 0);
      if (h !== 0) draw?.(h);
      return h;
    },
    size: (h, width, height, dpr) =>
      void surface.surface_resize({ link: ui, handle: h }, width, height, dpr),
    close: (h) => {
      drop(ui, h);
      closed?.(h);
    },
  };
}
