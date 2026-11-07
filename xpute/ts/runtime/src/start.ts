// @xpute/runtime/start.ts

/**
 * A page's core, in a worker of its own so a turn holds none of the UI's
 * events: the worker's module calls `serve`, and the UI holds the link across
 * the port. A native app's bundler takes start.native.ts in its place.
 */

import { connect, type Hello } from "./remote.ts";
import type { Link, Manifest, Ui } from "./runtime.ts";
import type { Grant } from "./sched/policy.ts";

/** The UI's link to the core of `m`, which `worker` serves, once it rose. */
export async function start<G extends Grant>(
  m: Manifest<G>,
  ui: Ui,
  worker: () => Worker,
): Promise<Link> {
  const w = worker();
  const { port1, port2 } = new MessageChannel();
  const { link, rose } = connect(port1, m.machine.slot_bytes, ui);
  w.postMessage({ xpute: port2, cmds: ui.cmds } satisfies Hello, [port2]);
  await rose;
  return link;
}
