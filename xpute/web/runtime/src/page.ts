// @xpute/web-runtime/page.ts

/**
 * The UI's end of the guest's worker, as @xpute/native-runtime is a device's
 * of its runtime: the UI opens its end of the link (`open_remote`), mounts on
 * it what it serves, hears the guest's stop and its trap (`stopped_on`,
 * `trap_on`), and starts the guest (`start`). A page has one worker, as a
 * process has one runtime. What the worker posts is a message by its kind,
 * the runtime's own and a host's or the program's alike (`message_on`), and
 * the last of each kind is kept, so that one who begins to hear a kind hears
 * the last told before, as a device's runtime keeps its stop and its trap.
 */

import { Remote, type Stop } from "@xpute/link";
import { tell_life } from "./life.ts";
import type { Instantiate, Stopped, Trap } from "./message.ts";

/** A message the worker posts the page. */
interface Message {
  kind: string;
}

/** The worker, and the port it takes with the instantiation. */
let worker: Worker;
let port: MessagePort;

/** The last message of each kind, and who hears them. */
const last = new Map<string, Message>();
const hearers = new Set<(m: Message) => void>();

/** Opens the guest's module worker at `url`, and the UI's end of the link on
 * one port of a channel to it; the other goes to the worker as the guest
 * starts (`start`). */
export function open_remote(url: string | URL): Remote {
  worker = new Worker(url, { type: "module" });
  worker.onmessage = ({ data: m }: MessageEvent<Message>) => {
    last.set(m.kind, m);
    for (const hear of hearers) hear(m);
  };
  const { port1, port2 } = new MessageChannel();
  port = port2;
  return new Remote(port1);
}

/** Adds `hear` to what is told each message of `kind` the worker posts, and
 * tells it the last posted before, as a device's runtime tells its stop. */
export function message_on<M extends Message>(
  kind: M["kind"],
  hear: (m: M) => void,
): { remove(): void } {
  const f = (m: Message) => {
    if (m.kind === kind) hear(m as M);
  };
  hearers.add(f);
  const m = last.get(kind);
  if (m !== undefined) f(m);
  return { remove: () => hearers.delete(f) };
}

/** Adds `hear` to what is told the guest's stop, as the link has it
 * (`Stopped`), as @xpute/native-runtime `stopped_on` tells a device's. */
export function stopped_on(hear: (stop: Stop) => void): { remove(): void } {
  return message_on<Stopped>("stopped", (m) => hear(m.stop));
}

/** Adds `hear` to what is told that the guest trapped, with what stopped it
 * (`Trap`), as @xpute/native-runtime `trap_on` tells a device's. */
export function trap_on(hear: (err: string) => void): { remove(): void } {
  return message_on<Trap>("trap", (m) => hear(m.err));
}

/** Starts the guest with what the UI serves, once it is mounted on `remote`,
 * as @xpute/native-runtime `start` does on a device: the worker is sent the
 * guest's `module`, fetched with `init`, the UI's port, the named handles and
 * prefixes of what the UI serves, and `more`, the program's own fields, which
 * its worker reads with them (`Instantiate`). The guest is told the page's
 * lifecycle from here on (`tell_life`), as a device's module tells its
 * runtime the app's. */
export function start(
  remote: Remote,
  module: string,
  init: RequestInit | undefined,
  more: object = {},
): void {
  const ask: Instantiate = {
    module,
    init,
    port,
    named: remote.named,
    prefixes: remote.prefixes,
  };
  worker.postMessage({ ...more, ...ask }, [port]);
  tell_life(remote);
}
