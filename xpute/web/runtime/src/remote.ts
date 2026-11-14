// @xpute/web-runtime/remote.ts

/** The link's side of a page's UI across its port (@xpute/link remote.ts), as
 * xpute-native-runtime remote.rs is a device's: what the UI serves mounted on
 * the link as stand-ins that tell it across the port, and what it sends taken
 * from the port onto the link. A page's worker dies with the page, so a
 * reloaded page meets a new worker and link over a new port, and no
 * generation is counted here: a device's runtime outlives its UI and counts
 * them (xpute-native-runtime remote.rs `accept`). */

import type { U8Array } from "@xpute/kit/abi/array.ts";
import { ERRNO_EINPROGRESS } from "@xpute/kit/status/errno.spec.ts";
import {
  DROP,
  type Handle,
  type Method,
  type Port,
  type Side,
  type Tag,
} from "@xpute/link";
import type { Later, Prefixes, ToLink, ToUi } from "@xpute/link/remote.ts";

/** What takes the things the UI gives beside the link, by the handle it
 * gives each under, and hears the handle dropped. */
export interface Taker {
  give(h: Handle, thing: unknown): void;
  take(h: Handle): void;
}

/** Serves `link` to the UI across `port`: the interfaces the UI serves
 * mounted under their named handles (`Remote.named`), the methods `later`
 * names answered later, an opener at each of its `prefixes`, for its
 * interface (`Remote.prefixes`), and what the UI gives beside the link to
 * `taker`. */
export function serve(
  port: Port,
  link: Side,
  named: readonly Handle[],
  prefixes: Prefixes,
  later: Later,
  taker: Taker,
): void {
  const tell = (m: ToUi) => port.postMessage(m);
  // Tells the UI a method it answers later, and the guest's cancel of it.
  const forward = (h: Handle, m: Method, tag: Tag, args: U8Array | null) => {
    tell({ h, m, tag, pkt: args === null ? null : args.slice() });
    link.on_cancel(tag, () => tell({ cancel: tag }));
    return ERRNO_EINPROGRESS;
  };
  for (const h of named) {
    link.mount_named(h, {
      call(m, tag, args) {
        if (later.some(([lh, lm]) => lh === h && lm === m))
          return forward(h, m, tag, args);
        tell({ h, m, tag: null, pkt: args === null ? null : args.slice() });
        return 0;
      },
      drop() {},
    });
  }
  for (const [i, p] of prefixes) {
    link.mount(i, p, (uri, h) => {
      tell({ open: h, uri });
      return {
        call: (m, tag, args) => forward(h, m, tag, args),
        drop: () => tell({ drop: h }),
      };
    });
  }
  port.onmessage = (e) => {
    const m = e.data as ToLink;
    if ("give" in m) return taker.give(m.give, m.thing);
    if ("answer" in m) return link.task_return(m.answer, m.result, m.data);
    if (m.m === DROP) taker.take(m.h);
    // A packet past the cap is refused whole, EMSGSIZE.
    link.send(m.h, m.m, m.pkt, (result, data) =>
      tell({ tag: m.tag, result, data }),
    );
  };
}
