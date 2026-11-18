// @xpute/runtime-web/remote.ts

/** The worker's half of the UI's crossing, as xpute-runtime-native remote.rs
 * is the native runtime's: the runtime stands in for the UI on the guest's
 * link, what the UI implements mounted on the guest's end as stand-ins that
 * post each method to the UI across its port, each answered later by its tag,
 * and what the UI calls put on the guest's end and answered back (message.ts).
 * The guest's end issues the handle of an object the UI implements as the guest
 * opens it, and the UI holds it under that handle (page.ts). The UI attaches as
 * its port does, and its port is its connection for the worker's life: the web
 * worker dies with the page, so a reloaded page meets a new worker and link
 * over a new port, and no UI is dropped here, where the native runtime outlives
 * its UI and drops one as a new one attaches (xpute-runtime-native remote.rs
 * `accept`). */

import { ERRNO_EINPROGRESS } from "@xpute/kit/status/errno.spec.ts";
import {
  DROP,
  type Handle,
  type Method,
  type Side,
  type Tag,
} from "@xpute/link/link.ts";
import { type Lift } from "@xpute/link/cabi.ts";
import type { Prefixes, ToPage, ToWorker } from "./message.ts";

/** The least of a port the worker takes: what posts a message and what
 * hears one. A MessagePort is one. */
export interface Port {
  postMessage(message: unknown): void;
  onmessage: Hear["hear"] | null;
}

/** What a port calls with each message. It is a method's type, so that a
 * port whose handler takes a fuller event, as a MessagePort's does, fits. */
interface Hear {
  hear(e: { data: unknown }): void;
}

/** What takes the things the UI gives beside its calls, by the handle it
 * gives each under, and hears the handle dropped: the web's port moves
 * objects. */
export interface Taker {
  give(h: Handle, thing: unknown): void;
  take(h: Handle): void;
}

/** Attaches the UI across `port` to `link`, the guest's end: the interfaces
 * the UI implements mounted under their `named` handles, an opener at each of
 * its `prefixes`, for its interface, and what the UI gives beside its calls
 * to `taker`. */
export function attach(
  port: Port,
  link: Side,
  named: readonly Handle[],
  prefixes: Prefixes,
  taker: Taker,
): void {
  const post = (m: ToPage) => port.postMessage(m);
  // Posts the UI a method it answers later, and the guest's cancel of it.
  const forward = (h: Handle, m: Method, tag: Tag, args: Lift) => {
    const pkt = args.pkt();
    post({ kind: "request", tag, h, m, pkt: pkt && pkt.slice() });
    link.on_cancel(tag, () => post({ kind: "cancel", tag }));
    return ERRNO_EINPROGRESS;
  };
  for (const h of named) {
    link.mount_named(h, {
      call: (m, tag, args) => forward(h, m, tag, args),
      drop() {},
    });
  }
  prefixes.forEach(([i, p], k) => {
    link.mount(i, p, (uri, h) => {
      post({ kind: "open", h, k, uri });
      return {
        call: (m, tag, args) => forward(h, m, tag, args),
        drop: () => post({ kind: "drop", h }),
      };
    });
  });
  const hear: {
    [K in ToWorker["kind"]]: (m: Extract<ToWorker, { kind: K }>) => void;
  } = {
    // A packet past the cap is refused whole, EMSGSIZE.
    request: (m) => {
      if (m.m === DROP) taker.take(m.h);
      link.send(m.h, m.m, m.pkt, (result, pkt) =>
        post({ kind: "answer", tag: m.tag, result, pkt }),
      );
    },
    answer: (m) => link.task_return(m.tag, m.result, m.pkt),
    give: (m) => taker.give(m.h, m.thing),
  };
  port.onmessage = (e) => {
    const m = e.data as ToWorker;
    (hear[m.kind] as (m: ToWorker) => void)(m);
  };
}
