// @xpute/runtime/life.ts

/** The page's lifecycle, told to a core that serves it (wit/life.wit): hidden
 * on `visibilitychange` to hidden, the Page Lifecycle's last sure event, and
 * shown on visible. A core that serves none is told nothing. */

import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import * as life from "./abi/life.ts";
import type { Ref, Sender } from "./link.ts";

export async function tell_life(link: Sender): Promise<void> {
  let core: Ref;
  try {
    core = await life.open(link);
  } catch (e) {
    if (e instanceof XputeError && e.errnum === Errno.ENOENT) return;
    throw e;
  }
  document.addEventListener("visibilitychange", () => {
    void (document.visibilityState === "hidden" ? life.hidden(core) : life.shown(core));
  });
}
