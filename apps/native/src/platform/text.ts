// @pxl/native/platform/text.ts

/** Text through CoreText, called synchronously on the JS thread. */

import type { f32, u32 } from "@xpute/core/abi/word.ts";
import { PLATFORM } from "./module.ts";

export function textWidth(weight: u32, px: f32, text: string): f32 {
  return PLATFORM.textWidth(weight, px, text);
}

/** `rgba` is 0xRRGGBBAA; `out` is RGBA, top row first, not premultiplied. */
export function textRaster(weight: u32, px: f32, rgba: u32, text: string, x: f32, width: u32, height: u32, out: Uint8Array): void {
  PLATFORM.textRaster(weight, px, rgba, text, x, width, height, out);
}
