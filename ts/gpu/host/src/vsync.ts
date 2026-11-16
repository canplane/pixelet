// @pxl/gpu/host/src/vsync.ts

/** The display's vsync the host implements for the guest beside its GPU
 * (spec/gpu/wit/vsync.wit), on the platform's animation frames, as a
 * device's host takes the display link's (pxl-gpu-host vsync.rs). A frame's
 * start is the animation frame's timestamp, which lies on the line of
 * `performance.now()` in the same scope, the monotonic clock's (@xpute/host
 * clock.ts `mark`). The host asks for every animation frame while it is
 * mounted, as a display link calls at every frame, so each gap is one
 * frame's but across a pause, as while the page is hidden. */

import { mark } from "@xpute/host";
import { ERRNO_ECANCELED } from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import type { Side } from "@xpute/link/link.ts";
import * as vsync from "./abi/vsync.ts";

/** A waiting `next`: its `after`, and its answer. */
interface Wait {
  after: bigint;
  done: (frame: vsync.Frame) => void;
}

export function mount_vsync(side: Side): void {
  const waiting = new Set<Wait>();
  // The last animation frame's start, none before the first.
  let last: bigint | null = null;
  const on_frame = (timestamp: number) => {
    const at = mark(timestamp);
    const gap = last === null ? 0n : at - last;
    last = at;
    for (const wait of waiting) {
      if (wait.after > at) continue;
      waiting.delete(wait);
      wait.done({ at, gap });
    }
    requestAnimationFrame(on_frame);
  };
  requestAnimationFrame(on_frame);
  vsync.mount(side, {
    // A wait its caller cancels leaves the waiting and answers ECANCELED, as
    // the clock's does.
    next: (after, signal) =>
      new Promise((done, fail) => {
        const wait = { after, done };
        waiting.add(wait);
        signal.addEventListener("abort", () => {
          if (waiting.delete(wait)) fail(new XputeError(ERRNO_ECANCELED));
        });
      }),
  });
}
