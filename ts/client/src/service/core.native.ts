// @pxl/client/service/core.native.ts

/** The core is linked into the app and opened by @xpute/runtime's native
 * platform through xpute's module (@xpute/native), so the app gives no
 * source. Its memory is given in Rust and its GPU stream run there (pxl-core
 * lib.rs `core!`). */

import type { ModuleSource } from "@xpute/runtime";
import type { Config } from "./types.ts";

export function source(_cfg: Config): ModuleSource | undefined {
  return undefined;
}
