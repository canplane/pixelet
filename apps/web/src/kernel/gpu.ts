// @pxl/web/kernel/gpu.ts

import { create } from "zustand";

/** Whether the host asked for a GPU and none answered: the kernel, given none, cannot say it. */
const useGpu = create(() => ({ absent: false }));

export function gpuAbsent(absent: boolean): void {
  useGpu.setState({ absent });
}

export function useGpuAbsent(): boolean {
  return useGpu((s) => s.absent);
}
