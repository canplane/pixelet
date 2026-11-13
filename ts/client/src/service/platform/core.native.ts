// @pxl/client/service/platform/core.native.ts

/** The core is linked into the app, and xpute's native host opens it
 * through xpute's module (@xpute/native-runtime), so the app gives no
 * source. Rust gives the core its memory and runs its GPU stream (pxl-core
 * lib.rs `link!`). */

import type { ModuleSource } from "@xpute/link";
import type { Config } from "./types.ts";

export function source(_cfg: Config): ModuleSource | undefined {
  return undefined;
}
