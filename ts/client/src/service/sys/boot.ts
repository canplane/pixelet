// @pxl/client/service/sys/boot.ts

import { ListenerSet } from "@xpute/kit/lang/listener.ts";

/**
 * Where the client's boot stands:
 * - opening: the core's module is fetched and compiled, before any GPU is
 *   asked for;
 * - starting: the core runs, while a GPU and the source are asked for;
 * - loading: a GPU answered and the source is declared, and the world is
 *   made under a cover;
 * - started: the app shows;
 * - terminated: the core stopped for good.
 */
export const BOOT_OPENING = 0;
export const BOOT_STARTING = 1;
export const BOOT_LOADING = 2;
export const BOOT_STARTED = 3;
export const BOOT_TERMINATED = 4;
export type Boot =
  | typeof BOOT_OPENING
  | typeof BOOT_STARTING
  | typeof BOOT_LOADING
  | typeof BOOT_STARTED
  | typeof BOOT_TERMINATED;

let state: Boot = BOOT_OPENING;
const listeners = new ListenerSet();

export function bootState(): Boot {
  return state;
}

export function setBoot(next: Boot): void {
  state = next;
  listeners.notify();
}

export const watchBoot = listeners.watch;
