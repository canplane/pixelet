// @pxl/web/platform/web/text.ts

/**
 * Text in the platform's own family, as the kernel asks for it
 * (kernel/host.ts): how wide a line runs, and a line drawn. On the web, the
 * page's family on the browser's canvas, which falls back through every
 * script and draws color emoji as the page's own text does.
 */

import type { f32, u32 } from "@xpute/core/abi/word.ts";

let family: string | null = null;

/** A weight and a size in the page's own family, read off the page once. */
function css_font(weight: u32, px: f32): string {
  family ??= globalThis.document ? getComputedStyle(document.body).fontFamily : "sans-serif";
  return `${weight} ${px}px ${family}`;
}

let canvas: OffscreenCanvasRenderingContext2D | null | undefined;

/** A 2D context, where the runtime has one. */
function context(): OffscreenCanvasRenderingContext2D | null {
  if (canvas === undefined) canvas = typeof OffscreenCanvas === "undefined" ? null : new OffscreenCanvas(1, 1).getContext("2d", { willReadFrequently: true });
  return canvas;
}

/** How far a line of `text` runs at `weight` and `px`, in pixels. */
export function text_width(weight: u32, px: f32, text: string): f32 {
  const ctx = context();
  if (ctx === null) return text.length * px * 0.6;
  ctx.font = css_font(weight, px);
  return ctx.measureText(text).width;
}

/** A line of `text` at `weight` and `px` in `rgba` (0xRRGGBBAA), starting at
 * `x` with the middle of its em box on the middle row, drawn into `out`:
 * `width` by `height` pixels, RGBA a byte a channel, top row first, alpha not
 * premultiplied. */
export function text_raster(weight: u32, px: f32, rgba: u32, text: string, x: f32, width: u32, height: u32, out: Uint8Array): void {
  const ctx = context();
  if (ctx === null) return;
  const c = ctx.canvas;
  if (c.width < width) c.width = width;
  if (c.height < height) c.height = height;
  ctx.clearRect(0, 0, width, height);
  ctx.font = css_font(weight, px);
  ctx.fillStyle = `rgba(${rgba >>> 24},${(rgba >>> 16) & 0xff},${(rgba >>> 8) & 0xff},${(rgba & 0xff) / 255})`;
  ctx.textAlign = "left";
  ctx.textBaseline = "middle";
  ctx.fillText(text, x, height / 2);
  out.set(ctx.getImageData(0, 0, width, height).data);
}
