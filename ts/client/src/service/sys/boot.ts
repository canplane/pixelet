// @pxl/client/service/sys/boot.ts

import { ListenerSet } from "@xpute/kit/lang/listener.ts";
import { BOOT_OPENING, type Boot } from "@/abi/state.ts";

let state: Boot = BOOT_OPENING;
const listeners = new ListenerSet();

export function boot_state(): Boot {
  return state;
}

export function set_boot(next: Boot): void {
  state = next;
  listeners.notify();
}

export const watch_boot = listeners.watch;
