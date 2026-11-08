// @pxl/client/service/core/surface.ts

import type { u32 } from "@xpute/kit/abi/word.ts";
import { GpuCommand, type GpuProtocol } from "@xpute/runtime";
import { type SurfaceDriver } from "@/service/platform/types.ts";
import { ui } from "@/service/core/core.ts";

const gpu = (): GpuProtocol => ui;

export function surface_driver(draw?: (surface: u32) => void): SurfaceDriver {
  return {
    open: async () => {
      const surface = Math.max(
        0,
        await gpu().request(GpuCommand.GPU_SURFACE_OPEN),
      );
      if (surface !== 0) draw?.(surface);
      return surface;
    },
    size: (surface, width, height, dpr) =>
      gpu().send(GpuCommand.GPU_SURFACE_RESIZE, surface, width, height, dpr),
    close: (surface) => gpu().send(GpuCommand.GPU_SURFACE_CLOSE, surface),
    bind: (surface, canvas) => ui.bind(surface, canvas),
    unbind: (surface) => ui.unbind(surface),
  };
}
