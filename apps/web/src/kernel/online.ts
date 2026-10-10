// @pxl/web/kernel/online.ts

import { create } from "zustand";

/** Whether the device says it is online: the host's to know, as the GPU is, and the kernel's only as a read's failure. */
const useOnlineStore = create(() => ({ online: true }));

export function setOnline(online: boolean): void {
  useOnlineStore.setState({ online });
}

export function online(): boolean {
  return useOnlineStore.getState().online;
}

export function useOnline(): boolean {
  return useOnlineStore((s) => s.online);
}
