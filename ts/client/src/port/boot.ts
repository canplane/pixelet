// @pxl/client/port/boot.ts

/** Where the client's boot stands, kept for the shell to show; the boot
 * (runtime/boot.ts) moves it on. */

import { Listeners } from "@xpute/kit/lang/listeners.ts";
import { BOOT_OPENING, type Boot } from "@/abi/state.ts";

let state: Boot = BOOT_OPENING;
const listeners = new Listeners();

export function boot_state(): Boot {
  return state;
}

export function set_boot(next: Boot): void {
  state = next;
  listeners.notify();
}

export const watch_boot = listeners.watch;
