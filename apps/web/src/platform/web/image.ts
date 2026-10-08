// @pxl/web/platform/web/image.ts

import type { U8Array } from "@xpute/kit/abi/array.ts";
import type { u32 } from "@xpute/kit/abi/word.ts";
import { registerImage } from "@/platform/web/webgpu.ts";

/** False for a file the browser does not decode. Top row first, no color conversion, not premultiplied. */
export async function decodeImage(handle: u32, file: U8Array): Promise<boolean> {
  try {
    const image = await createImageBitmap(new Blob([file]), { colorSpaceConversion: "none", premultiplyAlpha: "none" });
    registerImage(handle, image);
    return true;
  } catch {
    return false;
  }
}
