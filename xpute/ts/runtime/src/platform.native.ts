// @xpute/runtime/platform.native.ts

/**
 * Opens a native app's core. The core is linked into the app and reached
 * through xpute's native module (@xpute/native), which gives its memory, and
 * it runs on its own thread (`Core.thread`). A page takes platform.ts
 * instead.
 */

import type { f64 } from "@xpute/kit/abi/word.ts";
import { XPUTE } from "@xpute/native";
import type { Core, Machine, ModuleSource } from "./runtime.ts";

/** Once the core has terminated, a turn answers NaN (xpute-runtime native
 * `run`). That is thrown here, as a page's module throws its trap. */
export function open(
  _machine: Machine,
  _source: ModuleSource | undefined,
): Promise<Core> {
  return Promise.resolve({
    mem: XPUTE.mem(),
    run(deadline_ms: f64) {
      const wake = XPUTE.run(deadline_ms);
      if (Number.isNaN(wake)) throw new Error("the core terminated");
      return wake;
    },
    answer_with: (answer) => XPUTE.callOn(answer),
    thread: {
      kick: (deadline_ms) => XPUTE.kick(deadline_ms),
      pause: () => XPUTE.pause(),
      resume: () => XPUTE.resume(),
      listen(heard) {
        XPUTE.addListener("onNotify", () => {
          XPUTE.answer();
          const [wake_ms, turn_ms] = XPUTE.turns();
          heard(wake_ms, turn_ms);
        });
        // The core outlives a JavaScript reload, and it tells nothing more
        // until the news it told before the reload has been read, so it is
        // read here.
        XPUTE.turns();
      },
    },
  });
}
