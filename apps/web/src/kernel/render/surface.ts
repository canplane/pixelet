// @pxl/web/kernel/render/surface.ts

/**
 * The canvases the kernel draws into
 * (crates/pxl/web/render/src/gpu/surface.rs). A canvas is the kernel's once
 * handed over: the page lays it out, and nothing here draws in it.
 */

import type { u32 } from "@xpute/core/abi/word.ts";
import { round } from "@xpute/core/math/scalar.ts";
import { Command } from "@/kernel/abi/cmd.spec.ts";
import { CORE_BUILD, packet } from "@/kernel/ipc.ts";
import { bind_surface, unbind_surface } from "@/kernel/render/gpu.ts";
import { dpr, request_frame } from "@/kernel/sched.ts";

/** What a surface shows: surface.rs's ROLE_WORLD and ROLE_CASE. */
export const SURFACE_WORLD = 0;
export const SURFACE_CASE = 1;

export interface OpenSurface {
  handle: u32;
  close: () => void;
}

/** Hands `canvas` to the kernel to draw `role` into, and follows its layout
 * box in device pixels. The handle is 0 when the kernel has no surface left,
 * and then nothing is drawn in it.
 *
 * The box is the layout's, not the bounding rectangle: a rectangle includes
 * transforms, so a canvas mounted inside a drawer that is sliding or scaling in
 * would report a size it only passes through, and no resize follows a
 * transform. Where the observer reports device pixels they are exact; where it
 * does not (Safari), the CSS box times the pixel ratio stands in, and a zoom —
 * which changes the ratio and not the box — is caught by the window's resize. */
export function open_surface(canvas: HTMLCanvasElement, role: u32): OpenSurface {
  const handle = CORE_BUILD.call(Command.SURFACE_CREATE, packet((b) => b.u32(role)), (result) => result >>> 0);
  if (handle === 0) return { handle, close: () => {} };
  bind_surface(handle, canvas);
  const send = (width: u32, height: u32) => {
    CORE_BUILD.send(0, Command.SURFACE_UPDATE, packet((b) => b.u32(handle).u32(width).u32(height).f64(dpr())));
    request_frame();
  };
  const send_box = () => send(round(canvas.clientWidth * dpr()), round(canvas.clientHeight * dpr()));
  const observer = new ResizeObserver((entries) => {
    const entry = entries[entries.length - 1];
    const exact = entry.devicePixelContentBoxSize?.[0];
    if (exact) send(exact.inlineSize, exact.blockSize);
    else send_box();
  });
  try {
    observer.observe(canvas, { box: "device-pixel-content-box" });
  } catch {
    observer.observe(canvas);
  }
  globalThis.addEventListener("resize", send_box);
  return {
    handle,
    close: () => {
      globalThis.removeEventListener("resize", send_box);
      observer.disconnect();
      CORE_BUILD.call(Command.SURFACE_DELETE, packet((b) => b.u32(handle)), (result) => result);
      unbind_surface(handle);
    },
  };
}
