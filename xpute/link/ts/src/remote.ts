// @xpute/link/remote.ts

/**
 * A UI on another thread than its guest's link, as a page's main thread is to
 * the guest's worker, reaches the link over a port its runtime hands in
 * (`Port`): `serve` on the link's side, `Remote` on the UI's. Packets cross the port as bytes. The UI
 * sends methods to the guest's objects and is answered by promise, with the
 * return data. The interfaces it serves the guest are mounted on the Remote
 * under their named handles before the guest boots, and the handles go with
 * the guest's module, so that `serve` mounts their stand-ins before the guest
 * calls them. A method WIT marks `async func` (the bindings' `LATER`) the UI
 * answers later, by its tag (`task_return`), and a cancel of it is told to the
 * UI (`on_cancel`); any other is told to the UI and answered 0 at once, since
 * the UI is across the port. The UI never waits, so it has no `call`.
 *
 * Beside them, a UI may serve the objects at the URIs under a prefix
 * (`mount`), opened across the port and answered later in all their methods,
 * as a device's UI serves the guest's files: the link's side mounts their
 * openers (`serve`, and xpute-native-runtime remote.rs `serve`), and tells
 * the UI each object's drop. The UI serves as the link does (link.ts
 * `Side`): only how an answer crosses differs. Its open was answered across
 * the port already, so an opener's refusal answers each method of what it
 * would have opened.
 *
 * Beside the link the UI gives a transferable, a canvas, under a handle the
 * worker's side issued, on the same port, so that it lands before what the
 * UI sends after it; it is taken back when the UI drops the handle.
 *
 * A port is one UI's for its whole life. A page's worker dies with the page,
 * so a reloaded page meets a new worker and link over a new port, and no
 * generation is counted here: a device's runtime outlives its UI and counts
 * them (xpute-native-runtime remote.rs `accept`).
 */

import type { U8Array } from "@xpute/kit/abi/array.ts";
import type { i32 } from "@xpute/kit/abi/word.ts";
import { ERRNO_EINPROGRESS, ERRNO_ENOENT, ERRNO_EOVERFLOW } from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import type { Token } from "./abi/types.ts";
import { Lower } from "./cabi.ts";
import {
  type Answered,
  type Args,
  DROP,
  type Handle,
  type Link,
  type LinkObject,
  type Method,
  type Packet,
  type Sender,
  Side,
  type Tag,
} from "./link.ts";

/** The least of a port the link's ends take: what posts a message, with the
 * things it transfers, and what hears one. A MessagePort is one. */
export interface Port {
  postMessage(message: unknown, transfer?: unknown[]): void;
  onmessage: Hear["hear"] | null;
}

/** What a port calls with each message. It is a method's type, so that a
 * port whose handler takes a fuller event, as a MessagePort's does, fits. */
interface Hear {
  hear(e: { data: unknown }): void;
}

/** What takes the things the UI gives beside the link, by the handle it
 * gives each under, and hears the handle dropped. */
export interface Taker {
  give(h: Handle, thing: unknown): void;
  take(h: Handle): void;
}

/** The methods of the named interfaces the UI serves that answer later, each
 * `[handle, method]` (the bindings' `LATER`). */
export type Later = readonly (readonly [Handle, Method])[];

/** What the UI sends the link's side: a method of the guest's, a thing
 * given beside the link, or its answer to a method it serves later. */
export type ToLink =
  | { tag: Tag; h: Handle; m: Method; pkt: U8Array | null }
  | { give: Handle; thing: unknown }
  | { answer: Tag; result: i32; data: U8Array | null };

/** What the link's side sends the UI: the answer to one of its methods, a
 * method of an object it serves (to answer by `tag`, or null where it is
 * told), an open of one under a prefix it serves, the guest's cancel of a
 * method it answers later, and a drop of an object it opened. */
export type ToUi =
  | { tag: Tag; result: i32; data: U8Array | null }
  | { h: Handle; m: Method; tag: Tag | null; pkt: U8Array | null }
  | { open: Handle; uri: readonly Token[] }
  | { cancel: Tag }
  | { drop: Handle };

/** Serves `link` to the UI across `port`: the interfaces the UI serves
 * mounted under their named handles (`Remote.named`), the methods `later`
 * names answered later, an opener at each of its `prefixes`
 * (`Remote.prefixes`), and what the UI gives beside the link to `taker`. */
export function serve(
  port: Port,
  link: Link,
  named: readonly Handle[],
  prefixes: readonly (readonly Token[])[],
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
        if (later.some(([lh, lm]) => lh === h && lm === m)) return forward(h, m, tag, args);
        tell({ h, m, tag: null, pkt: args === null ? null : args.slice() });
        return 0;
      },
      drop() {},
    });
  }
  for (const p of prefixes) {
    link.mount(p, (uri, h) => {
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
    // A packet past the room is refused whole, EMSGSIZE.
    link.send(m.h, m.m, m.pkt, (result, data) => tell({ tag: m.tag, result, data }));
  };
}

/** The UI's end of a link `serve` serves across a port. */
export class Remote extends Side<ReturnType<typeof opened>> implements Sender {
  private readonly _port: Port;
  private readonly _answers = new Map<Tag, (a: Answered) => void>();
  private _next = 0;

  constructor(port: Port) {
    super(opened());
    this._port = port;
    port.onmessage = (e) => this._hear(e.data as ToUi);
  }

  /** The named handles of the interfaces the UI serves, which `serve`
   * mounts. */
  get named(): Handle[] {
    return [...this._named.keys()];
  }

  /** The prefixes the UI opens the objects under (`mount`). */
  get prefixes(): (readonly Token[])[] {
    return this._mounts.map(([p]) => p);
  }

  private _hear(m: ToUi): void {
    if ("result" in m) {
      const a = this._answers.get(m.tag);
      this._answers.delete(m.tag);
      return a?.({ result: m.result, data: m.data });
    }
    if ("open" in m) {
      const open = this._opener(m.uri);
      const refused = open === undefined ? ERRNO_ENOENT : this._open(m.open, m.uri, open);
      if (refused !== 0) this._objects.put(m.open, { call: () => refused, drop() {} });
      return;
    }
    if ("cancel" in m) return this._cancel(m.cancel);
    if ("drop" in m) return void this._drop(m.drop);
    if (m.tag === null) return void this._call(m.h, m.m, 0, m.pkt);
    const reply = this._call(m.h, m.m, m.tag, m.pkt);
    if (reply === ERRNO_EINPROGRESS) return;
    const { result, data } = typeof reply === "number" ? { result: reply, data: null } : reply;
    this.task_return(m.tag, result, data);
  }

  send(h: Handle, m: Method, args: Args): Promise<Answered> {
    const tag = this._next++;
    return new Promise((resolve) => {
      this._answers.set(tag, resolve);
      this._port.postMessage({ tag, h, m, pkt: packet(args) } satisfies ToLink);
    });
  }

  /** Gives `thing` beside the link under `h`, a handle the worker's side
   * issued, until the UI drops `h`. */
  give(h: Handle, thing: unknown): void {
    this._port.postMessage({ give: h, thing } satisfies ToLink, [thing]);
  }

  /** Posts the answer to the guest's request `t` across the port. */
  protected _answer(t: Tag, result: i32, data: Packet): void {
    this._port.postMessage({ answer: t, result, data: packet(data) } satisfies ToLink);
  }
}

/** The objects the UI serves, under the handles the link's side issued as it
 * opened them. */
function opened() {
  const objects = new Map<Handle, LinkObject>();
  return {
    get: (h: Handle) => objects.get(h),
    put: (h: Handle, o: LinkObject) => void objects.set(h, o),
    remove: (h: Handle) => {
      const o = objects.get(h);
      objects.delete(h);
      return o;
    },
  };
}

/** The most bytes a packet holds, the Canonical ABI's largest list. */
const PKT_CAP = (1 << 28) - 1;

/** The bytes of the packet `args`: those `args` write, those it is, or none
 * for no args. A packet past the largest is EOVERFLOW. */
export function packet(args: Packet): U8Array | null {
  if (args === null || args instanceof Uint8Array) return args;
  for (let cap = 1 << 10; ; cap = Math.min(cap * 2, PKT_CAP)) {
    const buf = new Uint8Array(cap);
    const w = new Lower(buf, args[0]);
    args[1](w);
    const n = w.finish();
    if (n >= 0) return buf.subarray(0, n);
    if (cap === PKT_CAP) throw new XputeError(ERRNO_EOVERFLOW);
  }
}
