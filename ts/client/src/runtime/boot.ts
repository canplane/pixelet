// @pxl/client/runtime/boot.ts

/**
 * A page runs the core in a worker of its own (worker.ts), and reaches it
 * across a message port. The UI opens its end of the worker's link, hears
 * the core's stop, its trap and the GPU's state, mounts what it serves and
 * starts the core here (@xpute/web-runtime `open_remote`, `start`), as a
 * device's UI does its runtime's (boot.native.ts). The worker holds the
 * files, keeps a copy of the page's storage that it writes back here, and
 * tells the page the source's state. Where the boot stands is moved as on a
 * device (ui.ts).
 */

import { state_on } from "@xpute/gpu-web-host";
import {
  type Log,
  message_on,
  open_remote,
  start,
  stopped_on,
  trap_on,
} from "@xpute/web-runtime";
import { BUILD_ID } from "@/build.ts";
import { log } from "@/service/platform/log.ts";
import { stopped } from "@/service/sys/abort.ts";
import { GPU_STATE, set_gpu } from "@/service/sys/gpu.ts";
import { mirror_source } from "@/service/sys/fs/source.ts";
import { is_storage_key } from "@/service/sys/fs/storage.ts";
import * as storage from "@/service/platform/storage.ts";
import { serve_ui, start_client, trapped } from "@/runtime/ui.ts";
import type { KeepReport, Kept, SourceReport } from "@/runtime/worker.ts";

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
  const remote = open_remote(require.unstable_resolveWorker("./worker.ts"));
  stopped_on(stopped);
  trap_on(trapped);
  state_on((state) => set_gpu(GPU_STATE[state]));
  message_on<SourceReport>("source", (m) => mirror_source(m.state, m.decl));
  message_on<KeepReport>("keep", (m) => storage.set(m.key, m.val));
  message_on<Log>("log", (m) => log(`[worker] ${m.text}`));
  serve_ui(remote);
  // A worker cannot read the page's kept entries, so they go with the core's
  // instantiation.
  const { module, init } = source();
  const entries = storage
    .keys()
    .filter(is_storage_key)
    .map((key): [string, string] => [key, storage.get(key)!]);
  start(remote, module.href, init, { entries } satisfies Kept);
  start_client();
}
