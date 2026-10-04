// @pxl/web/kernel/render/surface.ts

import type { f64, u32 } from "@xpute/core/abi/word.ts";
import { Command, encode_surface_close, encode_surface_open, encode_surface_resize } from "@/kernel/abi/cmd.spec.ts";
import { CORE_DOORBELL } from "@/kernel/mem.ts";
import { request_frame } from "@/kernel/sched.ts";

/** 0 when the kernel has none left. */
export function surface_open(role: u32): u32 {
  return CORE_DOORBELL.call(Command.SURFACE_OPEN, encode_surface_open({ role }), (result) => result >>> 0);
}

export function surface_size(surface: u32, width: u32, height: u32, dpr: f64): void {
  CORE_DOORBELL.send(0, Command.SURFACE_RESIZE, encode_surface_resize({ surface, width, height, device_pixel_ratio: dpr }));
  request_frame();
}

export function surface_close(surface: u32): void {
  CORE_DOORBELL.call(Command.SURFACE_CLOSE, encode_surface_close({ surface }), (result) => result);
}
