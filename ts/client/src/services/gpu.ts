// @pxl/client/services/gpu.ts

/** The page's GPU (@xpute/gpu `WebGpu`), granted the kernel as its machine
 * asks (rust/client/kernel/machine.json `gpu`). */

import { WebGpu } from "@xpute/gpu/web.ts";

export const gpu = new WebGpu();
