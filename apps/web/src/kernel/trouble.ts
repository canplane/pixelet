// @pxl/web/kernel/trouble.ts

import { create } from "zustand";

/** What keeps the world from the screen that the host knows and the kernel cannot: it never started, or no GPU answered. */
export const enum HostTrouble {
  None = 0,
  BootFailed = 1,
  NoGpu = 2,
}

const useHostTrouble = create(() => ({ trouble: HostTrouble.None }));

export function hostTrouble(trouble: HostTrouble): void {
  useHostTrouble.setState({ trouble });
}

export function useTrouble(): HostTrouble {
  return useHostTrouble((s) => s.trouble);
}
