// @pxl/client/ui/feature/cover/standing.ts

import { useSyncExternalStore } from "react";

import { bootState, watchBoot } from "@/service/sys/boot.ts";
import { stopCode } from "@/service/sys/abort.ts";
import { useGpu, useCore, useOnline, useSource } from "@/ui/shared/watch.ts";
import type { Standing } from "./cover.ts";

export function useStanding(): Standing {
  return {
    boot: useSyncExternalStore(watchBoot, bootState),
    gpu: useGpu(),
    source: useSource(),
    online: useOnline(),
    stall: useCore((s) => s.stall),
    stopCode: useSyncExternalStore(watchBoot, stopCode),
    progress: useCore((s) => s.renderer_progress),
    area: useCore((s) => s.area_progress),
  };
}
