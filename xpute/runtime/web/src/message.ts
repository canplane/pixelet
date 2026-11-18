// @xpute/runtime-web/message.ts

/** The messages the page and the guest's worker share; a program's own
 * messages join these in its own unions. Beside them, the UI's port carries
 * the UI's crossing to the guest's link and back (remote.ts, page.ts): a
 * method called, its answer, and the opens, cancels and drops of what the UI
 * implements, by the tags and handles of the guest's end, and the things the UI
 * gives beside them. */

import type { U8Array } from "@xpute/kit/abi/array.ts";
import type { i32 } from "@xpute/kit/abi/word.ts";
import type { Handle, Method, Tag } from "@xpute/link/link.ts";
import type { Token } from "@xpute/link/abi/types.ts";
import type { UiStop } from "@xpute/host/runtime.ts";
import type { Level } from "@xpute/host/abi/log/log.enum.ts";

/** The prefixes the UI opens objects under, each `[handle, prefix]`, the
 * named handle of the interface whose resources they are, in the order the
 * UI mounted them. */
export type Prefixes = readonly (readonly [Handle, readonly Token[]])[];

/** What the page sends the worker to instantiate the guest (page.ts `start`):
 * the module, how to fetch it, the UI's port, the named handles of the
 * interfaces the UI implements, and the prefixes it opens the objects under,
 * each with its interface. */
export interface Instantiate {
  module: string;
  init: RequestInit | undefined;
  port: MessagePort;
  named: Handle[];
  prefixes: Prefixes;
}

/** A method `m` of the object `h`, sent across the port: the guest's, of an
 * object the UI implements, or the UI's, of the guest's; its request `tag` is
 * the caller's, which its answer names. */
export interface Request {
  kind: "request";
  tag: Tag;
  h: Handle;
  m: Method;
  pkt: U8Array | null;
}

/** The answer to the request `tag` of the side it goes to. */
export interface Answer {
  kind: "answer";
  tag: Tag;
  result: i32;
  pkt: U8Array | null;
}

/** Open the object at `uri` under `h`, the handle the guest's end issued, by
 * the UI's opener of the prefix `k` (`Prefixes`). */
export interface Open {
  kind: "open";
  h: Handle;
  k: number;
  uri: readonly Token[];
}

/** The guest canceled the request `tag` the UI owes. */
export interface Cancel {
  kind: "cancel";
  tag: Tag;
}

/** The guest dropped the object `h` the UI implements. */
export interface Drop {
  kind: "drop";
  h: Handle;
}

/** A thing the UI gives beside its calls under `h`, a handle the guest's end
 * issued, until the UI drops `h`. */
export interface Give {
  kind: "give";
  h: Handle;
  thing: unknown;
}

/** What the UI's port carries to the worker. */
export type ToWorker = Request | Answer | Give;

/** What the UI's port carries to the page. */
export type ToPage = Request | Answer | Open | Cancel | Drop;

/** A log record the worker's runtime wrote (worker.ts `log`): its level, its
 * context and its message. */
export interface Log {
  kind: "log";
  level: Level;
  context: string;
  msg: string;
}

/** A trap, out of an export or a failure of the host's own, its module not
 * loading or its memory refused among them, which stops the guest for good. */
export interface Trap {
  kind: "trap";
  err: string;
}

/** The guest stopped, with the report of the broken invariant: the guest's,
 * as the host that implements the stop hands it (worker.ts `stopped`), or the
 * host's own failure (worker.ts `hook`), which the runtime sends (worker.ts
 * `tell_stop`). */
export interface Stopped {
  kind: "stopped";
  stop: UiStop;
}
