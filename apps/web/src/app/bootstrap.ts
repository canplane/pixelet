// @pxl/web/app/bootstrap.ts

import { create } from "zustand";

import { boot_kernel } from "@/kernel/boot.ts";
import { bootstrapI18n } from "@/locale/i18n.ts";

const useBoot = create(() => ({ booted: false }));

export function bootstrapApp(): void {
  // The locale first: the host's boot messages are in it.
  bootstrapI18n();
  boot_kernel().then(
    () => {
      console.info("[boot] the kernel started");
      useBoot.setState({ booted: true });
    },
    (err) => console.error("[boot] the kernel did not start:", err),
  );
}

export function useBooted(): boolean {
  return useBoot((s) => s.booted);
}
