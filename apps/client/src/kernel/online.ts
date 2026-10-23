// @pxl/client/kernel/online.ts

import { create } from "zustand";

/** Whether the device says it is online: the host's to know, as the GPU is, and the kernel's only as a read's failure. */
const useOnlineStore = create(() => ({ online: true }));

export function setOnline(online: boolean): void {
  useOnlineStore.setState({ online });
}

export function online(): boolean {
  return useOnlineStore.getState().online;
}

/** Resolved once the device says it is online: at once if it does, else on its word, with no timer. */
export function whenOnline(): Promise<void> {
  if (online()) return Promise.resolve();
  return new Promise((resolve) => {
    const stop = useOnlineStore.subscribe((s) => {
      if (!s.online) return;
      stop();
      resolve();
    });
  });
}

export function useOnline(): boolean {
  return useOnlineStore((s) => s.online);
}
