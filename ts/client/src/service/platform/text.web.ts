// @pxl/client/service/platform/text.web.ts

/**
 * Text measured and drawn on the browser's canvas, which falls back through
 * every script and draws color emoji as the page does. It has no twin here: the
 * native text is the platform module's (Text.swift, Text.kt), which
 * pxl-runtime's text.rs implements, so service/core/text.ts imports it by
 * name.
 */

import type { f32, u32 } from "@xpute/kit/abi/word.ts";
import { ERRNO_ENOSYS } from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";

function face(weight: u32, px: f32): string {
  return `${weight} ${px}px system-ui`;
}

let canvas: OffscreenCanvasRenderingContext2D | null | undefined;

/** The canvas text is set on. A place with none answers ENOSYS, as a native
 * runtime whose system text is not yet given does (pxl-runtime text.rs), and
 * nothing is made up in its place. */
function context(): OffscreenCanvasRenderingContext2D {
  if (canvas === undefined) {
    canvas =
      typeof OffscreenCanvas === "undefined"
        ? null
        : (new OffscreenCanvas(1, 1).getContext("2d", {
            willReadFrequently: true,
          }) as OffscreenCanvasRenderingContext2D | null);
  }
  if (canvas === null) throw new XputeError(ERRNO_ENOSYS);
  return canvas;
}

export function width(weight: u32, px: f32, text: string): f32 {
  const ctx = context();
  ctx.font = face(weight, px);
  return ctx.measureText(text).width;
}

/** `rgba` is 0xRRGGBBAA. `out` receives RGBA pixels, top row first, with
 * alpha not premultiplied. */
export function raster(
  weight: u32,
  px: f32,
  rgba: u32,
  text: string,
  x: f32,
  width: u32,
  height: u32,
  out: Uint8Array,
): void {
  const ctx = context();
  const c = ctx.canvas;
  if (c.width < width) c.width = width;
  if (c.height < height) c.height = height;
  ctx.clearRect(0, 0, width, height);
  ctx.font = face(weight, px);
  ctx.fillStyle = `rgba(${rgba >>> 24},${(rgba >>> 16) & 0xff},${
    (rgba >>> 8) & 0xff
  },${(rgba & 0xff) / 255})`;
  ctx.textAlign = "left";
  ctx.textBaseline = "middle";
  ctx.fillText(text, x, height / 2);
  out.set(ctx.getImageData(0, 0, width, height).data);
}
