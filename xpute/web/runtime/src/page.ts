// @xpute/web-runtime/page.ts

/** The page's end of the guest's worker. */

import { Remote } from "@xpute/link";

/** Opens the module worker at `url`, a channel to it, and the UI's end of
 * the link on one of its ports; the other goes to the worker with the
 * instantiation (`Instantiate`). */
export function open_worker(url: string | URL): {
  worker: Worker;
  remote: Remote;
  port: MessagePort;
} {
  const worker = new Worker(url, { type: "module" });
  const { port1, port2 } = new MessageChannel();
  return { worker, remote: new Remote(port1), port: port2 };
}
