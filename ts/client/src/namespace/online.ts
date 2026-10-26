// @pxl/client/namespace/online.ts

import { Listeners } from "@xpute/kit/lang/listeners.ts";

/** Whether the device says it is online: the host's to know, as the GPU is, and the guest's only as a read's failure. */
let state = true;
const listeners = new Listeners();

export function online(): boolean {
  return state;
}

export function set_online(online: boolean): void {
  state = online;
  listeners.notify();
}

export const watch_online = listeners.watch;

/** Resolved once the device says it is online: at once if it does, else on its word, with no timer. */
export function when_online(): Promise<void> {
  if (state) return Promise.resolve();
  return new Promise((resolve) => {
    const stop = watch_online(() => {
      if (!state) return;
      stop();
      resolve();
    });
  });
}
