// @pxl/client/service/platform/core.native.ts

/** The core and xpute's runtime are linked into the app, and the runtime
 * runs the core on a thread of its own, reached through xpute's module
 * (@xpute/native-runtime) as a page's worker is across a port. The UI serves
 * the core the files and the objects here, over the core's memory, where the
 * guest lends their buffers (`link`). */

import type { Link, Remote } from "@xpute/link";
import { bytes, start } from "@xpute/native-runtime";
import type { Core } from "./types.ts";

/** The core in the runtime the boot opened, reached through `remote`;
 * `start` starts the runtime with what is mounted on `remote`. */
export function core_of(remote: Remote): Core & {
  link: Pick<Link, "bytes" | "mount" | "mount_named" | "task_return" | "on_cancel">;
} {
  return {
    remote,
    start: () => start(remote),
    link: {
      bytes,
      mount: (prefix, o) => remote.mount(prefix, o),
      mount_named: (h, o) => remote.mount_named(h, o),
      task_return: (t, result, data) => remote.task_return(t, result, data),
      on_cancel: (t, stop) => remote.on_cancel(t, stop),
    },
  };
}
