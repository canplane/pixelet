// @pxl/native/platform/image.ts

/**
 * An image the renderer uploads, decoded where the device is: on a device, by
 * the core itself, off the JavaScript thread, into the pixels its GPU copies
 * from (crates/pxl/web/render/src/gpu/api/device.rs).
 */

import type { U8Array } from "@xpute/core/abi/array.ts";
import type { u32 } from "@xpute/core/abi/word.ts";
import { PLATFORM } from "./module.ts";

/** Decodes a file's image and holds it for the renderer as `handle`, the
 * kernel's own: false for a file the core does not decode. The file's own
 * pixels, top row first, no color conversion and no premultiplied alpha. */
export async function decodeImage(handle: u32, file: U8Array): Promise<boolean> {
  return (await PLATFORM.imageHold(handle, file)) !== 0;
}
