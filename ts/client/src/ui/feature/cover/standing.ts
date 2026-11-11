// @pxl/client/ui/feature/cover/standing.ts

import { useSyncExternalStore } from "react";

import { GroundStatus } from "@/abi/cmd.spec.ts";
import { boot_state, watch_boot } from "@/service/sys/boot.ts";
import { stop_code } from "@/service/sys/abort.ts";
import { useGpu, useCore, useOnline, useSource } from "@/ui/shared/watch.ts";
import type { Standing } from "./cover.ts";

export function useStanding(): Standing {
  return {
    boot: useSyncExternalStore(watch_boot, boot_state),
    gpu: useGpu(),
    source: useSource(),
    online: useOnline(),
    stall: useCore((s) => s.stall),
    stopCode: useSyncExternalStore(watch_boot, stop_code),
    progress: useCore((s) => s.renderer_progress),
    area: useCore((s) => s.area_progress),
    placed: useCore((s) => s.pivot.status !== GroundStatus.Idle),
  };
}
