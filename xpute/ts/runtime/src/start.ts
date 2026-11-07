// @xpute/runtime/start.ts

/**
 * A page's core, in a worker of its own so a turn holds none of the UI's
 * events: the worker's module calls `serve`, and the UI holds the link across
 * the port. A native app's bundler takes start.native.ts in its place.
 */

import { connect, type Hello } from "./remote.ts";
import type { Link, Manifest, Ui } from "./runtime.ts";
import type { Grant } from "./sched/policy.ts";

/** The UI's link to the core of `m`, which `worker` serves, once it rose. A
 * worker that fails before that refuses the link; one that fails after is
 * the core's stop, since a worker's failure says nothing on the port. */
export async function start<G extends Grant>(
  m: Manifest<G>,
  ui: Ui,
  worker: () => Worker,
): Promise<Link> {
  const w = worker();
  const { port1, port2 } = new MessageChannel();
  const { link, rose, stop } = connect(port1, m.machine.slot_bytes, ui);
  const failed = new Promise<never>((_, fail) => {
    w.onerror = w.onmessageerror = (e: Event) =>
      fail(new Error(`the core's worker: ${e.type}`));
  });
  w.postMessage({ xpute: port2, cmds: ui.cmds } satisfies Hello, [port2]);
  await Promise.race([rose, failed]);
  w.onerror = w.onmessageerror = stop;
  return link;
}
