// @pxl/web/app/bootstrap.ts

import { create } from "zustand";

import { boot_kernel } from "@/kernel/boot.ts";
import { bootstrapI18n } from "@/locale/i18n.ts";

export const enum Boot {
  Starting = 0,
  Started = 1,
  Failed = 2,
}

const useBootStore = create(() => ({ boot: Boot.Starting }));

export function bootstrapApp(): void {
  // The locale first: the host's boot messages are in it.
  bootstrapI18n();
  boot_kernel().then(
    () => {
      console.info("[boot] the kernel started");
      useBootStore.setState({ boot: Boot.Started });
    },
    (err) => {
      console.error("[boot] the kernel did not start:", err);
      useBootStore.setState({ boot: Boot.Failed });
    },
  );
}

export function useBoot(): Boot {
  return useBootStore((s) => s.boot);
}
