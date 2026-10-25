// @pxl/client/abi/surface.ts

import type { f64, u32 } from "@xpute/kit/abi/word.ts";
import type { SurfaceDriver } from "@/bracket.tsx";
import { Command, encode_surface_close, encode_surface_open, encode_surface_resize } from "@/abi/cmd.spec.ts";
import { GUEST_DOORBELL, HOST } from "@/host/mount.ts";

/** 0 when the kernel has none left. */
function surface_open(role: u32): u32 {
  return GUEST_DOORBELL.call(Command.SURFACE_OPEN, encode_surface_open(GUEST_DOORBELL, role), (result) => result >>> 0);
}

function surface_size(surface: u32, width: u32, height: u32, dpr: f64): void {
  HOST.send(Command.SURFACE_RESIZE, encode_surface_resize(GUEST_DOORBELL, surface, width, height, dpr));
}

function surface_close(surface: u32): void {
  GUEST_DOORBELL.call(Command.SURFACE_CLOSE, encode_surface_close(GUEST_DOORBELL, surface), (result) => result);
}

/** A surface of `role` (api.spec.ts `surface_role`) as the place drives it. */
export function surface_driver(role: u32): SurfaceDriver {
  return { open: () => surface_open(role), size: surface_size, close: surface_close };
}
