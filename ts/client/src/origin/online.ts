// @pxl/client/origin/online.ts

import { Listeners } from "@xpute/kit/lang/listeners.ts";

/** Whether the device says it is online. Like the GPU's state, this is the
 * client's to know. The core learns of it only as a read that fails. */
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

/** Resolves once the device says it is online: at once if it already does,
 * or else when it says so, with no timer. */
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
