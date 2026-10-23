// @pxl/web/text.ts

/**
 * Text measured and drawn on the browser's canvas, which falls back through
 * every script and draws color emoji as the page does.
 */

import type { f32, u32 } from "@xpute/kit/abi/word.ts";

let family: string | null = null;

function cssFont(weight: u32, px: f32): string {
  family ??= globalThis.document ? getComputedStyle(document.body).fontFamily : "sans-serif";
  return `${weight} ${px}px ${family}`;
}

let canvas: OffscreenCanvasRenderingContext2D | null | undefined;

function context(): OffscreenCanvasRenderingContext2D | null {
  if (canvas === undefined) canvas = typeof OffscreenCanvas === "undefined" ? null : new OffscreenCanvas(1, 1).getContext("2d", { willReadFrequently: true });
  return canvas;
}

export function textWidth(weight: u32, px: f32, text: string): f32 {
  const ctx = context();
  if (ctx === null) return text.length * px * 0.6;
  ctx.font = cssFont(weight, px);
  return ctx.measureText(text).width;
}

/** `rgba` is 0xRRGGBBAA; `out` is RGBA, top row first, not premultiplied. */
export function textRaster(weight: u32, px: f32, rgba: u32, text: string, x: f32, width: u32, height: u32, out: Uint8Array): void {
  const ctx = context();
  if (ctx === null) return;
  const c = ctx.canvas;
  if (c.width < width) c.width = width;
  if (c.height < height) c.height = height;
  ctx.clearRect(0, 0, width, height);
  ctx.font = cssFont(weight, px);
  ctx.fillStyle = `rgba(${rgba >>> 24},${(rgba >>> 16) & 0xff},${(rgba >>> 8) & 0xff},${(rgba & 0xff) / 255})`;
  ctx.textAlign = "left";
  ctx.textBaseline = "middle";
  ctx.fillText(text, x, height / 2);
  out.set(ctx.getImageData(0, 0, width, height).data);
}
