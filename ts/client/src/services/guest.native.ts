// @pxl/client/services/guest.native.ts

/** The guest is linked into the app and opened by @xpute/host's native
 * platform through xpute's module (@xpute/native), so the app gives no
 * source. Its memory is given in Rust and its GPU stream run there (pxl-kernel
 * lib.rs `guest!`). */

import type { ModuleSource } from "@xpute/host";
import type { Config } from "./types.ts";

export function source(_cfg: Config): ModuleSource | undefined {
  return undefined;
}
