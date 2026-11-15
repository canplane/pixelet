// @xpute/web-runtime/message.ts

/** The messages the page and the guest's worker share; a program's own
 * messages join these in its own unions. */

import type { Remote, Stop } from "@xpute/link";

/** What the page sends the worker to instantiate the guest (page.ts
 * `start`): the module, how to fetch it, the UI's port, the named handles of
 * the interfaces the UI serves, and the prefixes it opens the objects under,
 * each with its interface. */
export interface Instantiate {
  module: string;
  init: RequestInit | undefined;
  port: MessagePort;
  named: Remote["named"];
  prefixes: Remote["prefixes"];
}

/** A line of the worker's console. */
export interface Log {
  kind: "log";
  text: string;
}

/** A trap, out of an export or a failure of the host's own, its module not
 * loading or its memory refused among them, which stops the guest for good. */
export interface Trap {
  kind: "trap";
  err: string;
}

/** The guest stopped, with the report of the broken invariant: the guest's,
 * as the host that serves the stop hands it (@xpute/web-host `serve_stop`),
 * or the host's own failure (worker.ts `hook`), which the runtime sends
 * (worker.ts `tell_stop`). */
export interface Stopped {
  kind: "stopped";
  stop: Stop;
}
