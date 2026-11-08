// @xpute/runtime/start.ts

/**
 * Starts a page's core in a worker of its own, so that a turn never holds up
 * the UI's events. The worker's module calls `serve`, and the UI holds the
 * link across the port. A native app's bundler takes start.native.ts in this
 * file's place.
 */

import { connect, type Hello } from "./remote.ts";
import type { Link, Manifest, Ui } from "./runtime.ts";
import type { Grant } from "./sched/policy.ts";

/** Resolves to the UI's link to the core of `m`, which `worker` serves, once
 * the core is up. A worker that fails before then rejects the link. A worker
 * that fails later is taken as the core's stop, since a worker's failure
 * says nothing on the port. */
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
