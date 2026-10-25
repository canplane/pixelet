// @pxl/native/guest.ts

/** The guest linked into the app, reached through the native module: a memory buffer and a doorbell, as a page gets. */

import type { f64 } from "@xpute/kit/abi/word.ts";
import type { Guest } from "@pxl/client/bracket.tsx";
import { PLATFORM } from "./module.ts";

/** The plan is not read here: the linked guest's memory is given in Rust
 * (pxl-native lib.rs `guest`), and its GPU stream played there (surface.rs). */
export function open(): Promise<Guest> {
  const buffer = PLATFORM.mem();
  PLATFORM.intr(0);
  return Promise.resolve({ memory: buffer, intr: (quota_ms: f64) => PLATFORM.intr(quota_ms) });
}
