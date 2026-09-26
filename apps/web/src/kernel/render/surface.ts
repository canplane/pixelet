// @pxl/web/kernel/render/surface.ts

/**
 * The kernel's surfaces as a platform opens them
 * (crates/pxl/web/render/src/gpu/surface.rs): one made for a role, sized in
 * device pixels as its box is laid out, and let go. What a surface is bound
 * to — a canvas handed over, a view's layer — is the platform's
 * (platform/web/surface.tsx, a device's surface).
 */

import type { f64, u32 } from "@xpute/core/abi/word.ts";
import { Command, encode_surface_create, encode_surface_delete, encode_surface_update } from "@/kernel/abi/cmd.spec.ts";
import { CORE_DOORBELL } from "@/kernel/mem.ts";
import { request_frame } from "@/kernel/sched.ts";

/** A surface for `role`: its handle, 0 when the kernel has none left. */
export function surface_create(role: u32): u32 {
  return CORE_DOORBELL.call(Command.SURFACE_CREATE, encode_surface_create({ role }), (result) => result >>> 0);
}

/** The surface's box: `width` by `height` device pixels, `dpr` of them to a
 * CSS pixel. */
export function surface_size(surface: u32, width: u32, height: u32, dpr: f64): void {
  CORE_DOORBELL.send(0, Command.SURFACE_UPDATE, encode_surface_update({ surface, width, height, device_pixel_ratio: dpr }));
  request_frame();
}

export function surface_delete(surface: u32): void {
  CORE_DOORBELL.call(Command.SURFACE_DELETE, encode_surface_delete({ surface }), (result) => result);
}
