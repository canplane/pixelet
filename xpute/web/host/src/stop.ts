// @xpute/web-host/stop.ts

/** The stop the host serves the guest (wit/stop/stop.wit): the guest's report
 * of a broken invariant, logged and handed to `r`, before the guest stops.
 * The host's own failures are its runtime's to tell, as a stop and a trap
 * (@xpute/web-runtime worker.ts `hook`), as a native host's are
 * (xpute-native-runtime native.rs `hook`). */

import { strerror } from "@xpute/kit/status/error.ts";
import * as stop from "@xpute/link/abi/stop/stop.ts";
import type { Side, Stop } from "@xpute/link";

export function serve_stop(
  link: Pick<Side, "mount_named">,
  r: (s: Stop) => void,
): void {
  stop.mount(link, {
    stopped(errnum, file, line, n1, n2) {
      console.error(`[guest] stopped: ${strerror(errnum)} at ${file}:${line} (${n1}, ${n2})`);
      r({ errnum, file, line, n1, n2 });
    },
  });
}
