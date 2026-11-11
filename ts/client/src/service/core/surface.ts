// @pxl/client/service/core/surface.ts

import type { u32 } from "@xpute/kit/abi/word.ts";
import { gpu_exports } from "@xpute/gpu/abi.spec.ts";
import { Major } from "@/abi/cmd.spec.ts";
import { type SurfaceDriver } from "@/service/platform/types.ts";
import { ui } from "@/service/core/core.ts";
import { canvas_bind, canvas_unbind } from "@/service/platform/gpu.ts";

const gpu = () => gpu_exports(ui, Major.GPU);

export function surface_driver(draw?: (surface: u32) => void): SurfaceDriver {
  return {
    open: async () => {
      // A surface refused is none.
      const surface = await gpu()
        .gpu_surface_open()
        .catch(() => 0);
      if (surface !== 0) draw?.(surface);
      return surface;
    },
    size: (surface, width, height, dpr) =>
      gpu().gpu_surface_resize(surface, width, height, dpr),
    close: (surface) => gpu().gpu_surface_close(surface),
    bind: (surface, canvas) => canvas_bind(surface, canvas),
    unbind: (surface) => canvas_unbind(surface),
  };
}
