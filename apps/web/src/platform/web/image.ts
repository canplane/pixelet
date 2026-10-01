// @pxl/web/platform/web/image.ts

/**
 * An image the renderer uploads, decoded where the device is: on the web, by
 * the browser, into a bitmap the page's WebGPU copies from (webgpu.ts).
 */

import type { U8Array } from "@xpute/core/abi/array.ts";
import type { u32 } from "@xpute/core/abi/word.ts";
import { registerImage } from "@/platform/web/webgpu.ts";

/** Decodes a file's image and holds it for the renderer as `handle`, the
 * kernel's own: false for a file the browser does not decode. The file's own
 * pixels, top row first, no color conversion and no premultiplied alpha. */
export async function decodeImage(handle: u32, file: U8Array): Promise<boolean> {
  try {
    const image = await createImageBitmap(new Blob([file]), { colorSpaceConversion: "none", premultiplyAlpha: "none" });
    registerImage(handle, image);
    return true;
  } catch {
    return false;
  }
}
