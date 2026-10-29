// @pxl/client/shell/boot.ts

import { create } from "zustand";

import { installConsole } from "@/shell/debug/console.ts";
import * as kernel from "@/abi/boot.ts";
import * as locale from "@/shell/locale/i18n.ts";

export const enum Boot {
  Starting = 0,
  Started = 1,
  Failed = 2,
  /** Started, then stopped for good. */
  Stopped = 3,
}

const store = create<{ boot: Boot; stop: string | null }>()(() => ({
  boot: Boot.Starting,
  stop: null,
}));

export function boot(): void {
  // The locale first: the host's boot messages are in it.
  locale.boot();
  kernel.boot().then(
    () => {
      console.info("[boot] the kernel started");
      if (__IS_DEV__) installConsole();
      store.setState({ boot: Boot.Started });
      kernel.on_stop(() =>
        store.setState({ boot: Boot.Stopped, stop: kernel.stop_code() }),
      );
    },
    (err) => {
      console.error("[boot] the kernel did not start:", err);
      store.setState({ boot: Boot.Failed });
    },
  );
}

export function useBoot(): Boot {
  return store((s) => s.boot);
}

/** The stop code of a kernel stopped with one. */
export function useStop(): string | null {
  return store((s) => s.stop);
}
