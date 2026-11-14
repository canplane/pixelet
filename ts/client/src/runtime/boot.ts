// @pxl/client/runtime/boot.ts

/**
 * A page runs the core in a worker of its own (worker.ts), opened and started
 * here, and reaches it across a message port. The worker holds the files,
 * keeps a copy of the page's storage that it writes back here, and tells the
 * page the source's state, the GPU's and the core's stop. Where the boot
 * stands is moved as on a device (ui.ts).
 */

import { open_worker, tell_life } from "@xpute/web-runtime";
import { BUILD_ID } from "@/build.ts";
import { log } from "@/service/platform/log.ts";
import { stopped } from "@/service/sys/abort.ts";
import { set_boot, BOOT_FAILED } from "@/service/sys/boot.ts";
import { GPU_STATE, set_gpu } from "@/service/sys/gpu.ts";
import { mirror_source } from "@/service/sys/fs/source.ts";
import { is_storage_key } from "@/service/sys/fs/storage.ts";
import * as storage from "@/service/platform/storage.ts";
import { serve_ui, start_client, trapped } from "@/runtime/ui.ts";
import type { ToPage, ToWorker } from "@/runtime/worker.ts";

/** The module is served beside the page, from the app's public/, where
 * wasm-build.py writes it. It is asked for under the build's id, so that a
 * deployment's page never runs the previous deployment's core. A dev server
 * keeps one id while the module is rebuilt, so a development bundle also
 * refuses the cache: a cached module would run an older core under newer
 * glue. */
function source(): { module: URL; init: RequestInit | undefined } {
  return {
    module: new URL(`/core.wasm?v=${BUILD_ID}`, location.href),
    init: __DEV__ ? { cache: "no-store" } : undefined,
  };
}

export function boot(): void {
  // Metro bundles the worker through this call, and through no form of
  // `new Worker` that tree shaking also compiles (metro.config.ts).
  const { worker, remote, port } = open_worker(
    require.unstable_resolveWorker("./worker.ts"),
  );
  worker.onmessage = ({ data: m }: MessageEvent<ToPage>) => {
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
  // A worker cannot read the page's kept entries, so they go with the core's
  // instantiation.
  const { module, init } = source();
  const entries = storage
    .keys()
    .filter(is_storage_key)
    .map((key): [string, string] => [key, storage.get(key)!]);
  worker.postMessage(
    {
      kind: "instantiate",
      module: module.href,
      init,
      port,
      named: remote.named,
      prefixes: remote.prefixes,
      entries,
    } satisfies ToWorker,
    [port],
  );
  // The core is told the page's lifecycle; a device's reaches its runtime
  // natively.
  tell_life(remote);
  start_client();
}
