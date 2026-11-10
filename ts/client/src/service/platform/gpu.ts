// @pxl/client/service/platform/gpu.ts

import type { u32 } from "@xpute/kit/abi/word.ts";
import type { Canvas } from "@xpute/gpu/gpu.ts";
import { type CanvasBinder, WebGpu } from "@xpute/gpu/web.ts";

export const gpu = new WebGpu();

export const after_turn = () => gpu.after_turn();

let binder: CanvasBinder;

/** A page lends its canvases through this. */
export function set_canvas_binder(b: CanvasBinder): void {
  binder = b;
}

export const canvas_bind = (handle: u32, canvas: Canvas) =>
  binder.bind(handle, canvas);
export const canvas_unbind = (handle: u32) => binder.unbind(handle);
