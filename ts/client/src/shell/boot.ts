// @pxl/client/shell/boot.ts

/** Where the client's boot stands, as the shell reads it through the port: the
 * app boots the client (abi/boot.ts), and the shell only shows what came of it. */

import { useEffect, useSyncExternalStore } from "react";

import { Boot } from "@/abi/state.ts";
import { installConsole } from "@/shell/debug/console.ts";
import { boot_state, stop_code, watch_boot } from "@/port/shell.ts";

export function useBoot(): Boot {
  const boot = useSyncExternalStore(watch_boot, boot_state);
  // A developer's console over the kernel, once it runs.
  useEffect(() => {
    if (__IS_DEV__ && boot === Boot.Started) installConsole();
  }, [boot]);
  return boot;
}

/** The stop code of a kernel stopped with one. */
export function useStop(): string | null {
  return useSyncExternalStore(watch_boot, stop_code);
}
