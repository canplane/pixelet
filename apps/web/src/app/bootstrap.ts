// @pxl/web/app/bootstrap.ts

import { useSyncExternalStore } from "react";

import { boot_kernel } from "@/kernel/kernel.ts";
import { bootstrapI18n } from "@/locale/i18n.ts";

let booted = false;
const listeners = new Set<() => void>();

function settle(): void {
  booted = true;
  for (const l of listeners) l();
}

export function bootstrapApp(): void {
  // The locale first: the host's boot messages are in it.
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
