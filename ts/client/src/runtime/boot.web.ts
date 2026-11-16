// @pxl/client/runtime/boot.web.ts

/**
 * A page runs the core in a worker of its own (worker.ts), and reaches it
 * across a message port. The UI opens its end of the worker, hears
 * the core's stop, its trap and the GPU's state, mounts what it implements
 * and starts the core here (@xpute/runtime-web `open_ui`, `start`), as a
 * device's UI does its runtime's (boot.native.ts). The worker holds the
 * files, keeps a copy of the page's storage that it writes back here, and
 * tells the page the source's state. Where the boot stands is moved as on a
 * device (ui.ts).
 */

import type { GpuReport } from "@pxl/gpu/host";
import {
  type Log,
  message_on,
  open_ui,
  start,
  stopped_on,
  trap_on,
} from "@xpute/runtime-web";
import { BUILD_ID } from "@/build.ts";
import { stopped } from "@/service/sys/abort.ts";
import { GPU_STATE, set_gpu } from "@/service/sys/gpu.ts";
import { mirror_source } from "@/service/sys/fs/source.ts";
import { is_storage_key } from "@/service/sys/fs/storage.ts";
import * as storage from "@/service/platform/storage.ts";
import { tell_drive } from "@/service/platform/drive.ts";
import { mount_ui, start_client, trapped } from "@/runtime/ui.ts";
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
  const ui = open_ui(require.unstable_resolveWorker("./worker.ts"));
  stopped_on(stopped);
  trap_on(trapped);
  message_on<GpuReport>("gpu", (m) => set_gpu(GPU_STATE[m.state]));
  message_on<SourceReport>("source", (m) => mirror_source(m.state, m.decl));
  message_on<KeepReport>("keep", (m) => storage.set(m.key, m.val));
  // The worker writes its runtime's records itself; a build made for
  // measuring keeps them for the tool that drives the page as well.
  if (__PERF_DRIVE__)
    message_on<Log>("log", (m) => tell_drive(`[${m.context}] ${m.message}`));
  mount_ui(ui);
  // A worker cannot read the page's kept entries, so they go with the core's
  // instantiation.
  const { module, init } = source();
  const entries = storage
    .keys()
    .filter(is_storage_key)
    .map((key): [string, string] => [key, storage.get(key)!]);
  start(module.href, init, { entries } satisfies Kept);
  start_client();
}
