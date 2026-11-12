// @pxl/client/runtime/boot.ts

/**
 * A page opens the core in a worker of its own (worker.ts) and reaches it
 * across a message port. The worker holds the files, and keeps a copy of the
 * page's storage that it writes back here. The UI takes no part in the boot:
 * it reads where the boot stands (service/sys/boot.ts), and, once the core
 * has started, what the core shows it.
 */

import { Remote, type Stop } from "@xpute/runtime";
import { GroundStatus, type RefusedFile, type Snapshot } from "@/abi/overlay.ts";
import type { Source } from "@/abi/state.ts";
import { BOOT_FAILED, BOOT_LOADING, BOOT_STARTED, BOOT_STARTING, BOOT_TERMINATED } from "@/abi/state.ts";
import { set_online } from "@/service/origin/online.ts";
import { log } from "@/service/platform/log.ts";
import type { GpuState } from "@xpute/gpu/gpu.ts";
import { stop_code, stopped } from "@/service/sys/abort.ts";
import { boot_state, set_boot } from "@/service/sys/boot.ts";
import { GPU_STATE, set_gpu } from "@/service/sys/gpu.ts";
import { type Declaration, mirror_source } from "@/service/sys/fs/source.ts";
import { is_storage_key } from "@/service/sys/fs/storage.ts";
import { serve_overlay, open_core } from "@/service/core/core.ts";
import { SENSOR_LISTENER, serve_watch, tell_platform } from "@/service/core/sensor.ts";
import { listen } from "@/service/platform/sensor-listener.ts";
import { tell_dev } from "@/service/core/setting.ts";
import { cur, watch_snapshot } from "@/service/core/snapshot.ts";
import { source } from "@/service/platform/core.ts";
import * as online from "@/service/platform/online.ts";
import * as storage from "@/service/platform/storage.ts";

/** What the page sends the worker to open the core: the module, how to
 * fetch it, the UI's port, the prefixes of the objects the UI serves, and
 * the page's kept keys, which a worker cannot read. */
export interface Open {
  kind: "open";
  module: string;
  init: RequestInit | undefined;
  port: MessagePort;
  prefixes: Remote["prefixes"];
  keys: [string, string][];
}

/** What the page sends the worker past the open: the files the core refused,
 * as the snapshot names them, which the worker sets aside. */
export type ToWorker = Open | { kind: "refused"; files: readonly RefusedFile[] };

/** What the worker sends the page: the source's state, a kept key it wrote,
 * the GPU's state, a start that failed, the core's report before it stops,
 * its trap, and a line of the worker's console. */
export type ToPage =
  | { kind: "source"; state: Source; decl: Declaration | null }
  | { kind: "keep"; key: string; val: string }
  | { kind: "gpu"; state: GpuState }
  | { kind: "stop"; stop: Stop }
  | { kind: "failed"; err: string }
  | { kind: "trap"; err: string }
  | { kind: "log"; text: string };

/** Whether the world has arrived, or nothing of it will show: every pipeline
 * is made, and the view is drawn or lies past the service area. When locating
 * fails, the view is drawn at its default place. */
function loaded(s: Snapshot): boolean {
  return s.renderer_progress === 1 && (s.pivot.status === GroundStatus.Success || s.pivot.outside);
}

export function boot(): void {
  // Metro bundles the worker through this call, and through no form of
  // `new Worker` that tree shaking also compiles (metro.config.ts).
  const worker = new Worker(require.unstable_resolveWorker("./worker.ts"), {
    type: "module",
  });
  const tell = (m: ToWorker, move: Transferable[] = []) => worker.postMessage(m, move);
  worker.onmessage = ({ data: m }: MessageEvent<ToPage>) => {
    switch (m.kind) {
      case "source":
        return mirror_source(m.state, m.decl);
      case "keep":
        return storage.set(m.key, m.val);
      case "gpu":
        return set_gpu(GPU_STATE[m.state]);
      case "stop":
        return stopped(m.stop);
      case "log":
        return log(`[worker] ${m.text}`);
      case "failed":
        log(`[core] did not start: ${m.err}`);
        return set_boot(BOOT_FAILED);
      case "trap":
        log(`[core] stopped${stop_code() ? ` (${stop_code()})` : ""}: ${m.err}`);
        return set_boot(BOOT_TERMINATED);
    }
  };
  const { port1, port2 } = new MessageChannel();
  const { module, init } = source();
  const remote = new Remote(port1);
  serve_overlay(remote);
  serve_watch(remote);
  const keys = storage
    .keys()
    .filter(is_storage_key)
    .map((key): [string, string] => [key, storage.get(key)!]);
  tell(
    { kind: "open", module: module.href, init, port: port2, prefixes: remote.prefixes, keys },
    [port2],
  );
  watch_snapshot(() => {
    tell({ kind: "refused", files: cur().refused_files });
    if (boot_state() === BOOT_LOADING && loaded(cur())) set_boot(BOOT_STARTED);
  });
  online.watch(set_online);
  set_boot(BOOT_STARTING);
  // Once every open has answered, the device's readings are routed to the
  // core, the core is told the device's sensors and whether this is a
  // development build, and the UI calls it from LOADING on;
  // the app shows once the world has arrived (`loaded`).
  open_core(remote)
    .then(() => {
      // The device's readings go to the core from here on, before the core
      // hears that it has sensors and starts one.
      listen(SENSOR_LISTENER);
      return Promise.all([tell_platform(), tell_dev()]);
    })
    .then(
      () => {
        if (boot_state() !== BOOT_STARTING) return;
        set_boot(BOOT_LOADING);
        // The world may have arrived before every open answered.
        if (loaded(cur())) set_boot(BOOT_STARTED);
      },
      (err: unknown) => {
        console.error("[boot] the client did not start:", err);
        if (boot_state() === BOOT_STARTING) set_boot(BOOT_FAILED);
      },
    );
}
