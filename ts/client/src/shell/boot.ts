// @pxl/client/shell/boot.ts

/** Where the client's boot stands, as the shell reads it through the port: the
 * app boots the client (runtime/boot.ts), and the shell only shows what came of it. */

import { useEffect, useSyncExternalStore } from "react";

import { GroundStatus, Message } from "@/abi/cmd.spec.ts";
import { BOOT_LOADING } from "@/abi/state.ts";
import { installConsole } from "@/shell/debug/console.ts";
import { boot_state, stop_code, watch_boot } from "@/port/shell.ts";
import {
  useGpu,
  useKernel,
  useOnline,
  useSource,
} from "@/shell/store/watch.ts";
import type { Standing } from "./cover.ts";

/** All `coverOf` reads, gathered. */
export function useStanding(): Standing {
  const boot = useSyncExternalStore(watch_boot, boot_state);
  const stall = useKernel((s) => s.stall);
  // A developer's console over the kernel, once it runs.
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
    made: useKernel((s) => s.renderer_progress),
    area: useKernel((s) => s.area_progress),
    placed: useKernel((s) => s.pivot.status !== GroundStatus.Idle),
  };
}
