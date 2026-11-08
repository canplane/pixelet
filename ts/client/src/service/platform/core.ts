// @pxl/client/service/platform/core.ts

import type { ModuleSource } from "@xpute/runtime";
import { BUILD_ID, DEV_BUNDLE } from "@/build.ts";
import type { Config } from "./types.ts";

/** The module is served beside the page, from the app's public/, where
 * wasm-build.py writes it. It is asked for under the build's id, so that a
 * deployment's page never runs the previous deployment's core. A dev server
 * keeps one id while the module is rebuilt, so a development bundle also
 * refuses the cache: a cached module would run an older core under newer
 * glue. */
export function source(cfg: Config): ModuleSource {
  return {
    module: new URL(`/core.wasm?v=${BUILD_ID}`, location.href),
    bytes: cfg.bytes,
    init: DEV_BUNDLE ? { cache: "no-store" } : undefined,
  };
}
