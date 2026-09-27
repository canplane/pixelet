// @pxl/native/platform/text.ts

/**
 * Text in the platform's own family, as the kernel asks for it
 * (kernel/host.ts): how wide a line runs, and a line drawn. On a device, the
 * system font through CoreText (module.ts), called at
 * once on the JavaScript thread.
 */

import type { f32, u32 } from "@xpute/core/abi/word.ts";
import { PLATFORM } from "./module.ts";

/** How far a line of `text` runs at `weight` and `px`, in pixels. */
export function textWidth(weight: u32, px: f32, text: string): f32 {
  return PLATFORM.textWidth(weight, px, text);
}

/** A line of `text` at `weight` and `px` in `rgba` (0xRRGGBBAA), starting at
 * `x` with the middle of its em box on the middle row, drawn into `out`:
 * `width` by `height` pixels, RGBA a byte a channel, top row first, alpha not
 * premultiplied. */
export function textRaster(weight: u32, px: f32, rgba: u32, text: string, x: f32, width: u32, height: u32, out: Uint8Array): void {
  PLATFORM.textRaster(weight, px, rgba, text, x, width, height, out);
}
