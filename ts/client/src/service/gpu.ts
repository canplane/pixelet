// @pxl/client/service/gpu.ts

/** The page's GPU (@xpute/gpu `WebGpu`), granted the core as its machine
 * asks (rust/client/core/machine.json `gpu`). */

import { WebGpu } from "@xpute/gpu/web.ts";

export const gpu = new WebGpu();
