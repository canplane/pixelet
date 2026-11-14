// @xpute/web-runtime/message.ts

/** The messages the page and the guest's worker share; a program's own
 * messages join these in its own unions. */

import type { Remote, Stop } from "@xpute/link";

/** What the page sends the worker to instantiate the guest: the module, how
 * to fetch it, the UI's port, the named handles of the interfaces the UI
 * serves, and the prefixes it opens the objects under. */
export interface Instantiate {
  kind: "instantiate";
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

/** The error the guest's instantiation came to: its module did not load, or
 * its memory was refused. */
export interface InstantiateError {
  kind: "instantiate_error";
  err: string;
}

/** A trap out of an export, which stops the guest for good. */
export interface Trap {
  kind: "trap";
  err: string;
}

/** The guest stopped, with its report of the broken invariant, which the
 * program sends as the host hands it the report (@xpute/web-host
 * `serve_stop`). */
export interface Stopped {
  kind: "stopped";
  stop: Stop;
}
