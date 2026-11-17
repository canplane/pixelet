// @pxl/client/service/origin/online.ts

import { ListenerSet } from "@xpute/kit/lang/listener.ts";

/** Whether the device says it is online. Like the GPU's state, this is the
 * client's to know. The core learns of it only as a read that fails. */
let state = true;
const listeners = new ListenerSet();

export function online(): boolean {
  return state;
}

export function setOnline(online: boolean): void {
  state = online;
  listeners.notify();
}

export const watchOnline = listeners.watch;

export function whenOnline(): Promise<void> {
  if (state) return Promise.resolve();
  return new Promise((resolve) => {
    const stop = watchOnline(() => {
      if (!state) return;
      stop();
      resolve();
    });
  });
}
