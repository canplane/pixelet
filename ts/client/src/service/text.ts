// @pxl/client/service/text.ts

/**
 * Text measured and drawn on the browser's canvas, which falls back through
 * every script and draws color emoji as the page does.
 */

import type { f32, u32 } from "@xpute/kit/abi/word.ts";

/** The language the text is drawn in, which picks a Han character's form. */
let language = "";

export function setLanguage(lng: string): void {
  language = lng;
}

function face(weight: u32, px: f32): string {
  return `${weight} ${px}px system-ui`;
}

/** `lang` is in the HTML standard and not yet in TypeScript's DOM types. */
type TextContext = OffscreenCanvasRenderingContext2D & { lang: string };

let canvas: TextContext | null | undefined;

function context(): TextContext | null {
  if (canvas === undefined) {
    canvas =
      typeof OffscreenCanvas === "undefined"
        ? null
        : (new OffscreenCanvas(1, 1).getContext("2d", {
            willReadFrequently: true,
          }) as TextContext | null);
  }
  return canvas;
}

export function width(weight: u32, px: f32, text: string): f32 {
  const ctx = context();
  if (ctx === null) return text.length * px * 0.6;
  ctx.lang = language;
  ctx.font = face(weight, px);
  return ctx.measureText(text).width;
}

/** `rgba` is 0xRRGGBBAA; `out` is RGBA, top row first, not premultiplied. */
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
  if (ctx === null) return;
  const c = ctx.canvas;
  if (c.width < width) c.width = width;
  if (c.height < height) c.height = height;
  ctx.clearRect(0, 0, width, height);
  ctx.lang = language;
  ctx.font = face(weight, px);
  ctx.fillStyle = `rgba(${rgba >>> 24},${(rgba >>> 16) & 0xff},${
    (rgba >>> 8) & 0xff
  },${(rgba & 0xff) / 255})`;
  ctx.textAlign = "left";
  ctx.textBaseline = "middle";
  ctx.fillText(text, x, height / 2);
  out.set(ctx.getImageData(0, 0, width, height).data);
}
