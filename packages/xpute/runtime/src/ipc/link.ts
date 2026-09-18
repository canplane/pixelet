// @xpute/runtime/ipc/link.ts

/**
 * One end of a boundary that carries frames by transfer, and the runtime
 * that serves a command port on it.
 *
 * Inside a page nothing on this boundary is lost or reordered, so there is
 * no acknowledgement, deadline or retry here: a request with `ACKREQ` gets
 * exactly one reply, and correlating it is the caller's table, keyed by
 * tx_id. Reliability is decided at the network boundary only
 * (issue/20260913-boundaries-commands-and-what-comes-back-from-april.md,
 * "Where reliability and transactions are decided").
 */

import { Frame, FrameFlag, type FrameMessage } from "./frame.ts";
import type { CommandPort } from "./port.ts";

export type Link = {
  /** Transfers the message: the frame and any spill are detached here. */
  send(msg: FrameMessage): void;
  /** Receives messages; returns the unsubscribe. */
  on(receive: (msg: FrameMessage) => void): () => void;
};

/** What a Worker and a worker's global scope both are, for this purpose. */
export type MessageEndpoint = {
  postMessage(data: unknown, transfer: Transferable[]): void;
  addEventListener(type: "message", listener: (event: MessageEvent) => void): void;
  removeEventListener(type: "message", listener: (event: MessageEvent) => void): void;
};

function transfer_list(msg: FrameMessage): ArrayBuffer[] {
  return msg.spill ? [msg.frame, msg.spill] : [msg.frame];
}

/** A link over a Worker, or over a worker's own `self`. */
export function endpoint_link(endpoint: MessageEndpoint): Link {
  return {
    send(msg) {
      endpoint.postMessage(msg, transfer_list(msg));
    },
    on(receive) {
      const listener = (event: MessageEvent) => receive(event.data as FrameMessage);
      endpoint.addEventListener("message", listener);
      return () => endpoint.removeEventListener("message", listener);
    },
  };
}

/**
 * Two links joined in memory, with no thread between them — for tests, and
 * for running a worker's code where no worker is wanted. Delivery is a
 * structured clone with the same transfer list a worker would get, so the
 * sender's buffers are detached exactly as they would be, and it is
 * deferred to a microtask, so nothing on either side can come to depend on
 * a reply arriving within the call that sent it.
 */
export function virtual_link_pair(): [Link, Link] {
  const receivers: [((msg: FrameMessage) => void) | null, ((msg: FrameMessage) => void) | null] = [null, null];
  const end = (other: 0 | 1, self: 0 | 1): Link => ({
    send(msg) {
      const moved = structuredClone(msg, { transfer: transfer_list(msg) });
      queueMicrotask(() => receivers[other]?.(moved));
    },
    on(receive) {
      receivers[self] = receive;
      return () => {
        if (receivers[self] === receive) receivers[self] = null;
      };
    },
  });
  return [end(1, 0), end(0, 1)];
}

/**
 * Serves `port` on `link`: every request is dispatched, and a request with
 * `ACKREQ` is answered once, with `RES` set here and nowhere else. A reply
 * arriving on a serving end is not a request and is ignored.
 */
export function serve(link: Link, port: CommandPort): () => void {
  return link.on((msg) => {
    const flags = new Frame(msg.frame).flags;
    if (flags & FrameFlag.RES) return;
    const reply = port.dispatch(msg);
    if (!(flags & FrameFlag.ACKREQ)) return;
    const frame = new Frame(reply.frame);
    frame.flags = flags | FrameFlag.RES;
    link.send(reply);
  });
}
