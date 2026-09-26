// @pxl/web/app/bootstrap.ts

import { useSyncExternalStore } from "react";

import { host_boot } from "@platform/host.ts";
import { declareUnsupported } from "@/app/store/unsupported-store.ts";
import { bootstrapI18n } from "@/locale/i18n.ts";

/** Where the host's boot is: the map is drawn only once it has come up,
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
  host_boot().then(
    (missing) => {
      if (missing) {
        declareUnsupported(missing);
        settle("failed");
      } else {
        settle("ready");
      }
    },
    (err) => {
      console.error("[boot] the host did not start:", err);
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
