// @xpute/web-host/stop.ts

/** The stop the host serves the guest (wit/stop/stop.wit): the guest's report
 * of a broken invariant, logged and told the page by its runtime
 * (@xpute/web-runtime `tell_stop`), before the guest stops, as a native host's
 * is (xpute-native-host stop.rs). The host's own failures are its runtime's
 * to tell, as a stop and a trap (@xpute/web-runtime worker.ts `hook`). */

import { strerror } from "@xpute/kit/status/error.ts";
import * as stop from "@xpute/link/abi/stop/stop.ts";
import type { Side } from "@xpute/link";
import { tell_stop } from "@xpute/web-runtime";

export function serve_stop(link: Pick<Side, "mount_named">): void {
  stop.mount(link, {
    stopped(errnum, file, line, n1, n2) {
      console.error(
        `[guest] stopped: ${strerror(errnum)} at ${file}:${line} (${n1}, ${n2})`,
      );
      tell_stop({ errnum, file, line, n1, n2 });
    },
  });
}
