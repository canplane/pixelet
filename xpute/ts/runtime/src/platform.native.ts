// @xpute/runtime/platform.native.ts

/**
 * A native app's core: linked into the app and reached through xpute's
 * native module (@xpute/native), which gives its memory, and run on its own
 * thread (`Core.thread`). A page takes platform.ts.
 */

import type { f64 } from "@xpute/kit/abi/word.ts";
import { XPUTE } from "@xpute/native";
import type { Core, Machine, ModuleSource } from "./runtime.ts";

/** A turn answers NaN once the core has terminated (xpute-runtime native
 * `run`), which is thrown here as a page's module throws its trap. */
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
      listen(ack) {
        XPUTE.addListener("onNotify", () => {
          XPUTE.answer();
          const [wake_ms, turn_ms] = XPUTE.ack();
          ack(wake_ms, turn_ms);
        });
      },
    },
  });
}
