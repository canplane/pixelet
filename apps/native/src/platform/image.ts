// @pxl/native/platform/image.ts

/**
 * An image the renderer uploads, on a device: decoded by the core itself once
 * it draws, since the pixels it uploads are the file's own. Until then none
 * is held here, and the kernel hears each as refused.
 */

import type { u32 } from "@xpute/core/abi/word.ts";

export function hold_image(_res: Response): Promise<u32 | null> {
  return Promise.resolve(null);
}
