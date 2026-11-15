// @pxl/gpu/host/src/vsync.ts

/** The display's vsync the host serves the guest beside its GPU
 * (spec/gpu/wit/vsync.wit), on the platform's animation frames. */

import { ERRNO_ECANCELED } from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import type { Side } from "@xpute/link/link.ts";
import * as vsync from "../../link/src/abi/vsync.ts";

export function serve_vsync(side: Side): void {
  vsync.mount(side, {
    // A wait its caller cancels gives up its frame and answers ECANCELED, as
    // the clock's does.
    next: (signal) =>
      new Promise((done, fail) => {
        const frame = requestAnimationFrame(() => done());
        signal.addEventListener("abort", () => {
          cancelAnimationFrame(frame);
          fail(new XputeError(ERRNO_ECANCELED));
        });
      }),
  });
}
