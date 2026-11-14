// @xpute/web-host/stop.ts

/** The stop the host serves the guest (wit/stop/stop.wit): the guest's report
 * of a broken invariant, logged and handed to `report`, before the guest
 * stops. The host's own failures trap the guest and take the same path
 * (`hook`), as a native host's do (xpute-native-host stop.rs). */

import { Bug, frames } from "@xpute/kit/status/bug.ts";
import { ERRNO_ENOTRECOVERABLE } from "@xpute/kit/status/errno.spec.ts";
import { strerror } from "@xpute/kit/status/error.ts";
import * as stop from "@xpute/link/abi/stop/stop.ts";
import { type Side, set_hook, type Stop } from "@xpute/link";

/** The program's report, which both the guest's stop and the host's own
 * failures reach. */
let report: ((s: Stop) => void) | null = null;

export function serve_stop(
  link: Pick<Side, "mount_named">,
  r: (s: Stop) => void,
): void {
  report ??= r;
  stop.mount(link, {
    stopped(errnum, file, line, n1, n2) {
      console.error(`[guest] stopped: ${strerror(errnum)} at ${file}:${line} (${n1}, ${n2})`);
      r({ errnum, file, line, n1, n2 });
    },
  });
}

/** Makes a failure of the host's own a stop, as xpute-native-host stop.rs's
 * `hook` makes a panic one: a throw out of its answer to the guest's import,
 * which the link hands it (@xpute/link `set_hook`), and any the worker leaves
 * unhandled or uncaught, as a method answered later throws one. The link's
 * `trap` stops the guest, so that no answer the failed host owed leaves the
 * guest waiting, and where it did just now, the failure is logged and handed
 * to the report, with its errno, a bug's or ENOTRECOVERABLE, and where it was
 * thrown, before the page hears the trap. A throw after the guest trapped is
 * no failure of the host's. */
export function hook(link: Pick<Side, "trap">): void {
  const fail = (e: unknown) => {
    if (!link.trap(e)) return;
    const stack = e instanceof Error ? (e.stack ?? "") : "";
    const errnum = e instanceof Bug ? e.errnum : ERRNO_ENOTRECOVERABLE;
    const [file, line] =
      e instanceof Bug ? [e.file, e.line] : (frames(stack)[0] ?? ["", 0]);
    console.error(`[host] stopped: ${strerror(errnum)} at ${file}:${line}`);
    report?.({ errnum, file, line, n1: 0n, n2: 0n });
  };
  set_hook(fail);
  addEventListener("error", (ev) => fail(ev.error));
  addEventListener("unhandledrejection", (ev) => fail(ev.reason));
}
