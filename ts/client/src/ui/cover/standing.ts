// @pxl/client/ui/cover/standing.ts

/** Where the client's boot stands, as the UI reads it from the services. The
 * app boots the client (runtime/boot.ts), and the UI only shows what came of
 * it. */

import { useEffect, useSyncExternalStore } from "react";

import { GroundStatus, Message } from "@/abi/cmd.spec.ts";
import { BOOT_LOADING } from "@/abi/state.ts";
import { installConsole } from "@/ui/debug/console.ts";
import { boot_state, watch_boot } from "@/service/sys/boot.ts";
import { stop_code } from "@/service/sys/abort.ts";
import { useGpu, useCore, useOnline, useSource } from "@/ui/watch.ts";
import type { Standing } from "./cover.ts";

/** Gathers everything `coverOf` reads. */
export function useStanding(): Standing {
  const boot = useSyncExternalStore(watch_boot, boot_state);
  const stall = useCore((s) => s.stall);
  // In development, a console over the core is installed once the core runs.
  useEffect(() => {
    if (__IS_DEV__ && boot === BOOT_LOADING) installConsole();
  }, [boot]);
  useEffect(() => {
    if (stall === Message.WorldUnavailable) {
      console.error(
        "[read] a read failed past every retry; the space is given up",
      );
    }
  }, [stall]);
  return {
    boot,
    gpu: useGpu(),
    source: useSource(),
    online: useOnline(),
    stall,
    stopCode: useSyncExternalStore(watch_boot, stop_code),
    made: useCore((s) => s.renderer_progress),
    area: useCore((s) => s.area_progress),
    placed: useCore((s) => s.pivot.status !== GroundStatus.Idle),
  };
}
