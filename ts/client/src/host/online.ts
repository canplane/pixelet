// @pxl/client/host/online.ts

/** Whether the device says it is online: the host's to know, as the GPU is, and the guest's only as a read's failure. */
let state = true;
const listeners = new Set<() => void>();

export function online(): boolean {
  return state;
}

export function set_online(online: boolean): void {
  state = online;
  for (const listener of listeners) listener();
}

export function watch_online(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

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
