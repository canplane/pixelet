// @xpute/web-host/stop.ts

/** The stop the host serves the guest (wit/stop/stop.wit): the guest's report
 * of a broken invariant, logged and handed to `report`, before the guest
 * stops. */

import { strerror } from "@xpute/kit/status/error.ts";
import * as stop from "@xpute/link/abi/stop/stop.ts";
import type { Link, Stop } from "@xpute/link";

export function serve_stop(link: Pick<Link, "mount_named">, report: (s: Stop) => void): void {
  stop.mount(link, {
    stopped(errnum, file, line, n1, n2) {
      console.error(`[guest] stopped: ${strerror(errnum)} at ${file}:${line} (${n1}, ${n2})`);
      report({ errnum, file, line, n1, n2 });
    },
  });
}
