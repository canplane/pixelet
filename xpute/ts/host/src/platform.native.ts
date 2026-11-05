// @xpute/host/platform.native.ts

/**
 * A native app's guest: linked into the app and reached through xpute's
 * native module (@xpute/native), which gives its memory, and run on its own
 * thread (`Guest.thread`). A page takes platform.ts.
 */

import type { f64 } from "@xpute/kit/abi/word.ts";
import { XPUTE } from "@xpute/native";
import type { Guest, Machine, ModuleSource } from "./host.ts";

/** A turn answers NaN once the guest has terminated (xpute-host native
 * `run`), which is thrown here as a page's module throws its trap. */
export function open(
  _machine: Machine,
  _source: ModuleSource | undefined,
): Promise<Guest> {
  return Promise.resolve({
    mem: XPUTE.mem(),
    run(quota_ms: f64) {
      const wake = XPUTE.run(quota_ms);
      if (Number.isNaN(wake)) throw new Error("the guest terminated");
      return wake;
    },
    answer_with: (answer) => XPUTE.hostcallOn(answer),
    thread: {
      kick: (quota_ms) => XPUTE.kick(quota_ms),
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
