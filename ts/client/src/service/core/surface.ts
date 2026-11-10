// @pxl/client/service/core/surface.ts

import type { u32 } from "@xpute/kit/abi/word.ts";
import { Command as GpuCommand } from "@xpute/gpu/abi.spec.ts";
import { gpu_protocol } from "@xpute/gpu/gpu.ts";
import { Major } from "@/abi/cmd.spec.ts";
import { type SurfaceDriver } from "@/service/platform/types.ts";
import { ui } from "@/service/core/core.ts";
import { canvas_bind, canvas_unbind } from "@/service/platform/gpu.ts";

const gpu = () => gpu_protocol(ui, Major.GPU);

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
    bind: (surface, canvas) => canvas_bind(surface, canvas),
    unbind: (surface) => canvas_unbind(surface),
  };
}
