// @pxl/client/service/core/surface.ts

import type { u32 } from "@xpute/kit/abi/word.ts";
import { gpu_exports } from "@xpute/runtime";
import { type SurfaceDriver } from "@/service/platform/types.ts";
import { ui } from "@/service/core/core.ts";

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
