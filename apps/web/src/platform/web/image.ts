// @pxl/web/platform/web/image.ts

/**
 * An image the renderer uploads, decoded where the device is: on the web, by
 * the browser, into a bitmap the page's WebGPU copies from (render/gpu.ts).
 */

import type { u32 } from "@xpute/core/abi/word.ts";
import { register_image } from "@/kernel/render/gpu.ts";

let next_image: u32 = 1;

/** Decodes a response's image and holds it for the renderer: the handle the
 * kernel names it by. The file's own pixels, top row first, no color
 * conversion and no premultiplied alpha. */
export async function hold_image(res: Response): Promise<u32 | null> {
  const image = await createImageBitmap(await res.blob(), { colorSpaceConversion: "none", premultiplyAlpha: "none" });
  const handle = next_image++;
  register_image(handle, image);
  return handle;
}
