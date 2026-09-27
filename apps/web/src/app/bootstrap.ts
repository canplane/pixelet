// @pxl/web/app/bootstrap.ts

import { useSyncExternalStore } from "react";

import { boot_kernel } from "@/kernel/kernel.ts";
import { bootstrapI18n } from "@/locale/i18n.ts";

/** Whether the kernel has come up: the map is drawn only once it has, since
 * everything on it speaks to the kernel, and a boot that fails leaves it
 * undrawn. */
let booted = false;
const listeners = new Set<() => void>();

function settle(): void {
  booted = true;
  for (const l of listeners) l();
}

export function bootstrapApp(): void {
  // The locale first, because what the host may have to say about the
  // device is said in it.
  bootstrapI18n();
  boot_kernel().then(
    settle,
    (err) => console.error("[boot] the kernel did not start:", err),
  );
}

export function useBooted(): boolean {
  return useSyncExternalStore(
    (l) => {
      listeners.add(l);
      return () => listeners.delete(l);
    },
    () => booted,
  );
}
