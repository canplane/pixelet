// @xpute/host/stop.ts

/** The stop the host implements for the guest (wit/stop/stop.wit): the
 * guest's report of a broken invariant, handed to the host's runtime
 * (runtime.ts `stopped`), which reports it, before the guest stops, as a
 * native host's is (xpute-host stop.rs). The host's own failures are its
 * runtime's to tell, as a stop and a trap. */

import * as stop from "./abi/stop/stop.ts";
import type { Side } from "@xpute/link/link.ts";
import { runtime } from "./runtime.ts";

export function mount_stop(side: Pick<Side, "mount_named">): void {
  stop.mount(side, {
    stopped(errnum, file, line, n1, n2, msg) {
      runtime().stopped({ errnum, file, line, n1, n2, msg });
    },
  });
}
