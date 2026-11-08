// @pxl/client/service/sys/gpu.ts

/** This state is kept for the UI to show. The core hears of the device itself
 * on the GPU's own port (@xpute/gpu). */

import { ListenerSet } from "@xpute/kit/lang/listener.ts";
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
import type { GpuState } from "@xpute/runtime";

export const GPU_STATE: Readonly<Record<GpuState, Gpu>> = {
  present: GPU_PRESENT,
  waiting: GPU_WAITING,
  missing: GPU_MISSING,
  lost: GPU_LOST,
  unsupported: GPU_UNSUPPORTED,
  insecure: GPU_INSECURE,
};

let state: Gpu = GPU_ASKING;
const listeners = new ListenerSet();

export function gpu(): Gpu {
  return state;
}

export function set_gpu(gpu: Gpu): void {
  state = gpu;
  listeners.notify();
}

export const watch_gpu = listeners.watch;

/** Resolves once a GPU is present, and never where none is. */
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
