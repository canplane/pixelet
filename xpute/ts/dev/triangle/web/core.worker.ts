// @xpute/triangle/web/core.worker.ts

/** The core, its runtime and the GPU share this worker. The page sends the
 * port its canvas comes down. */

import { gpu_grant } from "@xpute/gpu/gpu.ts";
import { gpu_serve } from "@xpute/gpu/web.ts";
import { serve } from "@xpute/runtime";
import { MACHINE } from "../machine.spec.ts";
import { GPU_MAJOR, gpu, manifest } from "./core.ts";

onmessage = ({ data }: MessageEvent<{ gpu?: MessagePort }>) => {
  if (data.gpu) gpu_serve(data.gpu, gpu);
};

void serve(manifest).then((r) => gpu_grant(gpu, r, MACHINE.gpu!, GPU_MAJOR));
