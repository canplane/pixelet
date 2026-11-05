// @pxl/client/services/guest.ts

/** The kernel's module as a page fetches it (@xpute/host `boot`). A cached
 * module would run yesterday's guest under today's glue. */

import type { ModuleSource } from "@xpute/host";
import type { Config } from "./types.ts";
import { id } from "virtual:pxl-build";
import { DEV_BUNDLE } from "./bundle.ts";

/** Served beside the page (the app's public/, where build-wasm.py writes it),
 * asked for under the build's id, so a deployment's page never runs the guest
 * of the one before. */
export function source(cfg: Config): ModuleSource {
  return {
    module: new URL(`/guest.wasm?v=${id}`, location.href),
    bytes: cfg.bytes,
    init: DEV_BUNDLE ? { cache: "no-store" } : undefined,
  };
}
