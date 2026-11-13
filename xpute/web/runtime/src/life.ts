// @xpute/web-runtime/life.ts

/** The page's lifecycle, told to a guest that serves it (wit/life/life.wit):
 * hidden on `visibilitychange` to hidden, the Page Lifecycle's last sure event,
 * and shown on visible. A guest that serves none answers ENOENT, and is told
 * nothing more. */

import { Errno } from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import * as life from "@xpute/link/abi/life/life.ts";
import type { Sender } from "@xpute/link";

export function tell_life(link: Sender): void {
  document.addEventListener("visibilitychange", () => {
    const told = document.visibilityState === "hidden" ? life.hidden(link) : life.shown(link);
    told.catch((e: unknown) => {
      if (!(e instanceof XputeError && e.errnum === Errno.ENOENT)) throw e;
    });
  });
}
