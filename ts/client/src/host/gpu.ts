// @pxl/client/host/gpu.ts

import { create } from "zustand";

import type { GpuHeard } from "@/bracket.tsx";

/** What the host has of a GPU, which the guest, given none, cannot say: one, none yet while it asks again, none it will ask for again, or none again after one it drew with. */
export const enum Gpu {
  Present = 0,
  Waiting = 1,
  Missing = 2,
  Lost = 3,
}

/** A place's word for its GPU, as the host keeps it. */
export const GPU_STATE: Readonly<Record<Parameters<GpuHeard["state"]>[0], Gpu>> = { present: Gpu.Present, waiting: Gpu.Waiting, missing: Gpu.Missing, lost: Gpu.Lost };

const useGpuStore = create(() => ({ gpu: Gpu.Present }));

export function setGpu(gpu: Gpu): void {
  useGpuStore.setState({ gpu });
}

export function useGpu(): Gpu {
  return useGpuStore((s) => s.gpu);
}
