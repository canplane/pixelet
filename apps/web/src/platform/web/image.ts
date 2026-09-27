// @pxl/web/platform/web/image.ts

/**
 * An image the renderer uploads, decoded where the device is: on the web, by
 * the browser, into a bitmap the page's WebGPU copies from (webgpu.ts).
 */

import type { u32 } from "@xpute/core/abi/word.ts";
import { registerImage } from "@/platform/web/webgpu.ts";

let nextImage: u32 = 1;

/** Decodes a response's image and holds it for the renderer: the handle the
 * kernel names it by. The file's own pixels, top row first, no color
 * conversion and no premultiplied alpha. */
export async function holdImage(res: Response): Promise<u32 | null> {
  const image = await createImageBitmap(await res.blob(), { colorSpaceConversion: "none", premultiplyAlpha: "none" });
  const handle = nextImage++;
  registerImage(handle, image);
  return handle;
}
