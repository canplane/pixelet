// @pxl/web/kernel/gpu.ts

import { create } from "zustand";

/** What the host has of a GPU, which the kernel, given none, cannot say: one, none yet while it asks again, none it will ask for again, or none again after one it drew with. */
export const enum Gpu {
  Present = 0,
  Waiting = 1,
  Missing = 2,
  Lost = 3,
}

const useGpuStore = create(() => ({ gpu: Gpu.Present }));

export function setGpu(gpu: Gpu): void {
  useGpuStore.setState({ gpu });
}

export function useGpu(): Gpu {
  return useGpuStore((s) => s.gpu);
}
