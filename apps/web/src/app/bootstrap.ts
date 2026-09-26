// @pxl/web/app/bootstrap.ts

import { useSyncExternalStore } from "react";

import { gpu_missing } from "@platform/gpu.ts";
import { boot_kernel } from "@/kernel/kernel.ts";
import { declareUnsupported } from "@/app/store/unsupported-store.ts";
import { bootstrapI18n } from "@/locale/i18n.ts";

/** Where the kernel's boot is: the map is drawn only once it has come up,
 * since everything on it speaks to the kernel. A device that cannot draw
 * never boots one, and is shown why instead (app/unsupported.tsx). */
export type BootState = "loading" | "ready" | "failed";

let state: BootState = "loading";
const listeners = new Set<() => void>();

function settle(next: BootState): void {
  state = next;
  for (const l of listeners) l();
}

export function bootstrapApp(): void {
  // The locale first, because what the host may have to say about the
  // device is said in it.
  bootstrapI18n();
  // A platform that cannot draw is told so instead of being started, since
  // the kernel would otherwise fetch its core, open a world and raise toasts
  // about loading it, under a notice saying none of it can be shown.
  const missing = gpu_missing();
  if (missing) {
    declareUnsupported(missing);
    settle("failed");
    return;
  }
  boot_kernel().then(
    () => settle("ready"),
    (err) => {
      console.error("[boot] the kernel did not start:", err);
      settle("failed");
    },
  );
}

export function useBootState(): BootState {
  return useSyncExternalStore(
    (l) => {
      listeners.add(l);
      return () => listeners.delete(l);
    },
    () => state,
  );
}
