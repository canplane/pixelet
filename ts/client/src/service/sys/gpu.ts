// @pxl/client/service/sys/gpu.ts

/** This state is kept for the UI to show, as the page's worker reports it
 * (runtime/worker.ts). The core hears of the device itself on the link
 * (@xpute/gpu-web-host web.ts). */

import { ListenerSet } from "@xpute/kit/lang/listener.ts";
import type { GpuState } from "@xpute/gpu-link";

/**
 * What the client has of a GPU. The core cannot say this itself, because a
 * core given no GPU learns nothing of one:
 * - asking: a GPU is asked for the first time;
 * - present: there is one;
 * - waiting: there is none yet, and one is asked for again;
 * - missing: there is none, and none will be asked for again;
 * - unsupported: this browser has no WebGPU;
 * - insecure: the page is not a secure context, so it has no GPU.
 */
export const GPU_ASKING = 0;
export const GPU_PRESENT = 1;
export const GPU_WAITING = 2;
export const GPU_MISSING = 3;
export const GPU_UNSUPPORTED = 4;
export const GPU_INSECURE = 5;
export type Gpu =
  | typeof GPU_ASKING
  | typeof GPU_PRESENT
  | typeof GPU_WAITING
  | typeof GPU_MISSING
  | typeof GPU_UNSUPPORTED
  | typeof GPU_INSECURE;

export const GPU_STATE: Readonly<Record<GpuState, Gpu>> = {
  present: GPU_PRESENT,
  missing: GPU_MISSING,
  waiting: GPU_WAITING,
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
