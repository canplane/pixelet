// @pxl/native/guest.ts

/** The guest linked into the app, reached through the native module: a memory buffer and a doorbell, as a page gets. */

import type { f64 } from "@xpute/kit/abi/word.ts";
import type { Guest } from "@pxl/client/bracket.tsx";
import { PLATFORM } from "./module.ts";

/** The config is not read here: the linked guest's memory is given in Rust
 * (pxl-native lib.rs `guest`), and its GPU stream played there (surface.rs).
 * A call the kernel makes comes back to this thread while `run` waits (xpute-host native
 * `answer_with`). */
export function open(): Promise<Guest> {
  const mem = PLATFORM.mem();
  return Promise.resolve({
    mem,
    // NaN: the kernel stopped, as a module's trap throws on a page.
    run: (quota_ms: f64) => {
      const wake = PLATFORM.run(quota_ms);
      if (Number.isNaN(wake)) throw new Error("the kernel stopped");
      return wake;
    },
    answer_with: (answer) => PLATFORM.hostcallOn(answer),
  });
}
