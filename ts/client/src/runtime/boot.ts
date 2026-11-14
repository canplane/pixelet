// @pxl/client/runtime/boot.ts

/**
 * A page runs the core in a worker of its own (worker.ts), opened here and
 * started through the page's core (service/platform/core.ts). The worker holds
 * the files, keeps a copy of the page's storage that it writes back here, and
 * tells the page the source's state, the GPU's and the core's stop. Where the
 * boot stands is moved as on a device (ui.ts).
 */

import { open_worker, tell_life } from "@xpute/web-runtime";
import { log } from "@/service/platform/log.ts";
import { stopped } from "@/service/sys/abort.ts";
import { set_boot, BOOT_FAILED } from "@/service/sys/boot.ts";
import { GPU_STATE, set_gpu } from "@/service/sys/gpu.ts";
import { mirror_source } from "@/service/sys/fs/source.ts";
import { is_storage_key } from "@/service/sys/fs/storage.ts";
import { core_of } from "@/service/platform/core.ts";
import * as storage from "@/service/platform/storage.ts";
import { serve_ui, start_client, trapped } from "@/runtime/ui.ts";
import type { ToPage, ToWorker } from "@/runtime/worker.ts";

export function boot(): void {
  // Metro bundles the worker through this call, and through no form of
  // `new Worker` that tree shaking also compiles (metro.config.ts).
  const opened = open_worker(require.unstable_resolveWorker("./worker.ts"));
  const { remote, start } = core_of(opened);
  opened.worker.onmessage = ({ data: m }: MessageEvent<ToPage>) => {
    switch (m.kind) {
      case "source":
        return mirror_source(m.state, m.decl);
      case "keep":
        return storage.set(m.key, m.val);
      case "gpu":
        return set_gpu(GPU_STATE[m.state]);
      case "stopped":
        return stopped(m.stop);
      case "log":
        return log(`[worker] ${m.text}`);
      case "instantiate_error":
        log(`[core] did not start: ${m.err}`);
        return set_boot(BOOT_FAILED);
      case "trap":
        return trapped(m.err);
    }
  };
  serve_ui(remote);
  // The page's kept entries, which a worker cannot read, go ahead of the
  // core's module.
  const entries = storage
    .keys()
    .filter(is_storage_key)
    .map((key): [string, string] => [key, storage.get(key)!]);
  opened.worker.postMessage({ kind: "entries", entries } satisfies ToWorker);
  start();
  // The core is told the page's lifecycle; a device's reaches its runtime
  // natively.
  tell_life(remote);
  start_client();
}
