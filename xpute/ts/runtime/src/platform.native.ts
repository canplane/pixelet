// @xpute/runtime/platform.native.ts

/**
 * Opens a native app's core. The core is linked into the app and reached
 * through xpute's native module (@xpute/native), which gives its memory, and it
 * runs on its own thread (`Core.thread`). Its clock is xpute-runtime's native
 * `now`, which counts as the app's `performance.now()` does, so `now` is not
 * taken here. A page takes platform.ts instead.
 */

import type { f64 } from "@xpute/kit/abi/word.ts";
import { XPUTE } from "@xpute/native";
import type { Core, Machine, ModuleSource } from "./runtime.ts";

/** The display frame last offered, and the last a turn was granted. */
let offered: f64 = 0;
let granted: f64 = 0;
/** What waits to be offered a frame. */
const waiting: ((now: f64) => void)[] = [];

/** Offers each waiter the next display frame. While a turn granted has not
 * begun it takes all that is sent before it, and a frame offered and not taken
 * is offered no more, so in either case the waiter waits for the display's
 * next frame. */
function offer(): void {
  for (const run of waiting.splice(0)) {
    const frame = XPUTE.nextFrame(granted);
    if (frame === offered || granted > performance.now()) {
      requestAnimationFrame(() => next_frame(run));
      continue;
    }
    offered = frame;
    run(frame);
  }
}

/** Hands `run` the next display frame's time ahead of that frame, as soon as
 * this thread is free. The turn it grants waits on the core's own thread for
 * that frame, so what this thread is busy with does not move it. */
export function next_frame(run: (now: f64) => void): void {
  waiting.push(run);
  setImmediate(offer);
}

/** Once the core has terminated, a turn answers NaN (xpute-runtime native
 * `run`). That is thrown here, as a page's module throws its trap. */
export function open(
  _machine: Machine,
  _source: ModuleSource | undefined,
  _now: () => f64,
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
      // The deadline counts from now, and the turn rises at its frame.
      kick(deadline_ms) {
        granted = offered;
        XPUTE.kick(
          Math.max(0, deadline_ms - Math.max(0, granted - performance.now())),
          granted,
        );
      },
      pause: () => XPUTE.pause(),
      resume: () => XPUTE.resume(),
      listen(heard) {
        XPUTE.addListener("onNotify", () => {
          XPUTE.answer();
          const [wake_ms, turn_ms] = XPUTE.turns();
          heard(wake_ms, turn_ms);
          // An event of a native module's does not run what it left for
          // `setImmediate` on iOS until something else runs on this thread.
          offer();
        });
        // The core outlives a JavaScript reload, and it tells nothing more
        // until the news it told before the reload has been read, so it is
        // read here.
        XPUTE.turns();
      },
    },
  });
}
