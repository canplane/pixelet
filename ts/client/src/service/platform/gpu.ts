// @pxl/client/service/platform/gpu.ts

/** The page's GPU (@xpute/gpu `WebGpu`), granted to the core as its machine
 * asks (spec/client/core/sys/machine.json `gpu`). */

import { WebGpu } from "@xpute/gpu/web.ts";

export const gpu = new WebGpu();
