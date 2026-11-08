// @pxl/client/service/core.native.ts

/** The core is linked into the app, and @xpute/runtime's native platform
 * opens it through xpute's module (@xpute/native), so the app gives no
 * source. Rust gives the core its memory and runs its GPU stream (pxl-core
 * lib.rs `core!`). */

import type { ModuleSource } from "@xpute/runtime";
import type { Config } from "./types.ts";

export function source(_cfg: Config): ModuleSource | undefined {
  return undefined;
}
