// @pxl/web/app/bootstrap.ts

import { useSyncExternalStore } from "react";

import { gpuMissing } from "@platform/gpu.ts";
import { boot_kernel } from "@/kernel/kernel.ts";
import { declareUnsupported } from "@/app/store/unsupported-store.ts";
import { bootstrapI18n } from "@/locale/i18n.ts";

/** Whether the kernel has come up: the map is drawn only once it has, since
 * everything on it speaks to the kernel. A device that cannot draw never
 * boots one and is shown why instead (app/unsupported.tsx), and a boot that
 * fails leaves the map undrawn as well. */
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
  // A platform that cannot draw is told so instead of being started, since
  // the kernel would otherwise fetch its core, open a world and raise toasts
  // about loading it, under a notice saying none of it can be shown.
  const missing = gpuMissing();
  if (missing) {
    declareUnsupported(missing);
    return;
  }
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
