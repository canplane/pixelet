// @pxl/client/abi/render/surface.ts

import type { f64, u32 } from "@xpute/kit/abi/word.ts";
import { Command, encode_surface_close, encode_surface_open, encode_surface_resize } from "@/abi/cmd.spec.ts";
import { GUEST_DOORBELL } from "@/host/mem.ts";
import { send_next } from "@/host/sched.ts";

/** 0 when the kernel has none left. */
export function surface_open(role: u32): u32 {
  return GUEST_DOORBELL.call(Command.SURFACE_OPEN, encode_surface_open(GUEST_DOORBELL, role), (result) => result >>> 0);
}

export function surface_size(surface: u32, width: u32, height: u32, dpr: f64): void {
  send_next(Command.SURFACE_RESIZE, encode_surface_resize(GUEST_DOORBELL, surface, width, height, dpr));
}

export function surface_close(surface: u32): void {
  GUEST_DOORBELL.call(Command.SURFACE_CLOSE, encode_surface_close(GUEST_DOORBELL, surface), (result) => result);
}
