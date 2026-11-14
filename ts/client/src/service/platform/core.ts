// @pxl/client/service/platform/core.ts

/** A page runs the core in a worker of its own, which the boot opens
 * (@xpute/web-runtime `open_worker`) and reaches across a message port. */

import type { Remote } from "@xpute/link";
import type { Instantiate } from "@xpute/web-runtime";
import { BUILD_ID } from "@/build.ts";
import type { Core } from "./types.ts";

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

/** The core in the worker the boot opened, reached through `remote`, whose
 * other end is `port`; `start` hands the worker the core's module, the port,
 * and the named handles and prefixes mounted on `remote`. */
export function core_of({
  worker,
  remote,
  port,
}: {
  worker: Worker;
  remote: Remote;
  port: MessagePort;
}): Core {
  return {
    remote,
    start() {
      const { module, init } = source();
      const m: Instantiate = {
        kind: "instantiate",
        module: module.href,
        init,
        port,
        named: remote.named,
        prefixes: remote.prefixes,
      };
      worker.postMessage(m, [port]);
    },
  };
}
