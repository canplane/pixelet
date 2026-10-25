// @pxl/client/shell/bootstrap.ts

import { create } from "zustand";

import { installConsole } from "@/shell/debug/console.ts";
import { boot_kernel, on_kernel_stop, stop_code } from "@/abi/boot.ts";
import { bootstrapI18n } from "@/shell/locale/i18n.ts";

export const enum Boot {
  Starting = 0,
  Started = 1,
  Failed = 2,
  /** Started, then stopped for good. */
  Stopped = 3,
}

const useBootStore = create(() => ({ boot: Boot.Starting, stop: null as string | null }));

export function bootstrapApp(): void {
  // The locale first: the host's boot messages are in it.
  bootstrapI18n();
  boot_kernel().then(
    () => {
      console.info("[boot] the kernel started");
      if (__IS_DEV__) installConsole();
      useBootStore.setState({ boot: Boot.Started });
      on_kernel_stop(() => useBootStore.setState({ boot: Boot.Stopped, stop: stop_code() }));
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

/** The stop code of a kernel stopped with one. */
export function useStopCode(): string | null {
  return useBootStore((s) => s.stop);
}
