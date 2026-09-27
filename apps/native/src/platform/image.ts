// @pxl/native/platform/image.ts

/**
 * An image the renderer uploads, decoded where the device is: on a device, by
 * the core itself, off the JavaScript thread, into the pixels its GPU copies
 * from (crates/pxl/web/render/src/gpu/api/device.rs).
 */

import type { u32 } from "@xpute/core/abi/word.ts";
import { PLATFORM } from "./module.ts";

/** Decodes a response's image and holds it for the renderer: the handle the
 * kernel names it by, or none for a file the core does not decode. The
 * file's own pixels, top row first, no color conversion and no premultiplied
 * alpha. */
export async function holdImage(res: Response): Promise<u32 | null> {
  const handle = await PLATFORM.imageHold(new Uint8Array(await res.arrayBuffer()));
  return handle === 0 ? null : handle;
}
