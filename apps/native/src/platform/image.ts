// @pxl/native/platform/image.ts

/** Decoded by the core off the JS thread. */

import type { U8Array } from "@xpute/kit/abi/array.ts";
import type { u32 } from "@xpute/kit/abi/word.ts";
import { PLATFORM } from "./module.ts";

/** False for a file the core does not decode. Top row first, no color conversion, not premultiplied. */
export async function decodeImage(handle: u32, file: U8Array): Promise<boolean> {
  return (await PLATFORM.imageHold(handle, file)) !== 0;
}
