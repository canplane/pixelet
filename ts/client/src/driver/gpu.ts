// @pxl/client/driver/gpu.ts

import { Listeners } from "@xpute/kit/lang/listeners.ts";
import { Gpu } from "@/abi/state.ts";
import type { GpuHeard } from "@/bracket.tsx";

/** A place's word for its GPU, as the host keeps it. */
export const GPU_STATE: Readonly<
  Record<Parameters<GpuHeard["state"]>[0], Gpu>
> = {
  present: Gpu.Present,
  waiting: Gpu.Waiting,
  missing: Gpu.Missing,
  lost: Gpu.Lost,
};

let state = Gpu.Present;
const listeners = new Listeners();

export function gpu(): Gpu {
  return state;
}

export function set_gpu(gpu: Gpu): void {
  state = gpu;
  listeners.notify();
}

export const watch_gpu = listeners.watch;
