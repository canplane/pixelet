// @pxl/client/driver/gpu.ts

import { Listeners } from "@xpute/kit/lang/listeners.ts";
import {
  GPU_ASKING,
  GPU_LOST,
  GPU_MISSING,
  GPU_INSECURE,
  GPU_PRESENT,
  GPU_UNSUPPORTED,
  GPU_WAITING,
  type Gpu,
} from "@/abi/state.ts";
import type { GpuHeard } from "@/bracket.tsx";

/** A place's word for its GPU, as the driver keeps it. */
export const GPU_STATE: Readonly<
  Record<Parameters<GpuHeard["state"]>[0], Gpu>
> = {
  present: GPU_PRESENT,
  waiting: GPU_WAITING,
  missing: GPU_MISSING,
  lost: GPU_LOST,
  unsupported: GPU_UNSUPPORTED,
  insecure: GPU_INSECURE,
};

let state: Gpu = GPU_ASKING;
const listeners = new Listeners();

export function gpu(): Gpu {
  return state;
}

export function set_gpu(gpu: Gpu): void {
  state = gpu;
  listeners.notify();
}

export const watch_gpu = listeners.watch;

/** Settled once a GPU has answered; never where none does. */
export function gpu_present(): Promise<void> {
  return new Promise((resolve) => {
    if (state === GPU_PRESENT) return resolve();
    const stop = watch_gpu(() => {
      if (state !== GPU_PRESENT) return;
      stop();
      resolve();
    });
  });
}
