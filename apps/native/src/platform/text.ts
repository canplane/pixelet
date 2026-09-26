// @pxl/native/platform/text.ts

/**
 * Text in the platform's own family, as the kernel asks for it
 * (kernel/host.ts): how wide a line runs, and a line drawn. On a device, the
 * system font through CoreText (modules/pxl-surface's PxlText), called at
 * once on the JavaScript thread.
 */

import type { f32, u32 } from "@xpute/core/abi/word.ts";
import { requireNativeModule } from "expo";

interface PxlText {
  width(weight: u32, px: f32, text: string): f32;
  raster(weight: u32, px: f32, rgba: u32, text: string, x: f32, width: u32, height: u32, out: Uint8Array): void;
}

const TEXT: PxlText = requireNativeModule("PxlText");

/** How far a line of `text` runs at `weight` and `px`, in pixels. */
export function text_width(weight: u32, px: f32, text: string): f32 {
  return TEXT.width(weight, px, text);
}

/** A line of `text` at `weight` and `px` in `rgba` (0xRRGGBBAA), starting at
 * `x` with the middle of its em box on the middle row, drawn into `out`:
 * `width` by `height` pixels, RGBA a byte a channel, top row first, alpha not
 * premultiplied. */
export function text_raster(weight: u32, px: f32, rgba: u32, text: string, x: f32, width: u32, height: u32, out: Uint8Array): void {
  TEXT.raster(weight, px, rgba, text, x, width, height, out);
}
