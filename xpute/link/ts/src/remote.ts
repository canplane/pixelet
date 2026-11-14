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
 * as a device's UI serves the guest's files: the runtime's side mounts their
 * openers (xpute-native-runtime remote.rs `serve`).
 *
 * Beside the link the UI gives a transferable, a canvas, under a handle the
 * worker's side issued, on the same port, so that it lands before what the
 * UI sends after it; it is taken back when the UI drops the handle.
 */

import type { U8Array } from "@xpute/kit/abi/array.ts";
import type { i32 } from "@xpute/kit/abi/word.ts";
import { ERRNO_EINPROGRESS, ERRNO_EOVERFLOW, ERRNO_EPROTO } from "@xpute/kit/status/errno.spec.ts";
import { bug } from "@xpute/kit/status/bug.ts";
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
  type Opener,
  refusal_of,
  type Sender,
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
 * names answered later, and what the UI gives beside the link to `taker`. */
export function serve(port: Port, link: Link, named: readonly Handle[], later: Later, taker: Taker): void {
  const tell = (m: ToUi) => port.postMessage(m);
  for (const h of named) {
    link.mount_named(h, {
      call(m, tag, args) {
        const pkt = args === null ? null : args.slice();
        if (!later.some(([lh, lm]) => lh === h && lm === m)) {
          tell({ h, m, tag: null, pkt });
          return 0;
        }
        tell({ h, m, tag, pkt });
        link.on_cancel(tag, () => tell({ cancel: tag }));
        return ERRNO_EINPROGRESS;
      },
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
export class Remote implements Sender {
  /** The named handles of the interfaces the UI serves, which `serve`
   * mounts. */
  readonly named: Handle[] = [];
  /** The prefixes the UI opens the objects under (`mount`). */
  readonly prefixes: (readonly Token[])[] = [];
  private readonly _port: Port;
  private readonly _answers = new Map<Tag, (a: Answered) => void>();
  /** The objects the UI serves: its interfaces by their named handles, and
   * those it opened by theirs. */
  private readonly _objects = new Map<Handle, LinkObject>();
  private readonly _mounts: [readonly Token[], Opener][] = [];
  /** How each method the UI answers later stops if the guest cancels it. */
  private readonly _cancels = new Map<Tag, () => void>();
  private _next = 0;

  constructor(port: Port) {
    this._port = port;
    port.onmessage = (e) => this._hear(e.data as ToUi);
  }

  private _hear(m: ToUi): void {
    if ("result" in m) {
      const a = this._answers.get(m.tag);
      this._answers.delete(m.tag);
      return a?.({ result: m.result, data: m.data });
    }
    if ("open" in m) return this._open(m.open, m.uri);
    if ("cancel" in m) {
      const stop = this._cancels.get(m.cancel);
      this._cancels.delete(m.cancel);
      return stop?.();
    }
    if ("drop" in m) return void this._objects.delete(m.drop);
    const o = this._objects.get(m.h);
    if (m.tag === null) return void o?.call(m.m, 0, m.pkt);
    const reply = o === undefined ? ERRNO_EPROTO : o.call(m.m, m.tag, m.pkt);
    if (reply === ERRNO_EINPROGRESS) return;
    const { result, data } = typeof reply === "number" ? { result: reply, data: null } : reply;
    this.task_return(m.tag, result, data);
  }

  /** Opens the object at `uri` under `h`, with the opener of the longest
   * prefix it starts with. */
  private _open(h: Handle, uri: readonly Token[]): void {
    const starts = (p: readonly Token[]) => p.every((t, k) => t.tag === uri[k]?.tag && t.val === uri[k]?.val);
    const opener = this._mounts
      .filter(([p]) => starts(p))
      .reduce<[readonly Token[], Opener] | undefined>((a, b) => (a === undefined || b[0].length > a[0].length ? b : a), undefined);
    if (opener === undefined) bug(ERRNO_EPROTO);
    // The guest's open was answered across the port already, so an opener's
    // refusal is the answer to each method of what it would have opened.
    let o: LinkObject;
    try {
      o = opener[1](uri, h);
    } catch (e) {
      const refused = refusal_of(e);
      o = { call: () => refused };
    }
    this._objects.set(h, o);
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

  /** Serves `o` under its interface's named handle `h`, before the guest
   * boots. */
  mount_named(h: Handle, o: LinkObject): void {
    this._objects.set(h, o);
    this.named.push(h);
  }

  /** Serves the objects at the URIs that start with `prefix`, each opened by
   * `open`, before the guest boots. */
  mount(prefix: readonly Token[], open: Opener): void {
    this._mounts.push([prefix, open]);
    this.prefixes.push(prefix);
  }

  /** Answers the guest's request `t`, whose method the UI answers later,
   * with `result` and its return data (Link `task_return`). */
  task_return(t: Tag, result: i32, data: Args = null): void {
    this._cancels.delete(t);
    this._port.postMessage({ answer: t, result, data: packet(data) } satisfies ToLink);
  }

  /** Puts `stop` as how the request `t` the UI answers later stops if the
   * guest cancels it (Link `on_cancel`). */
  on_cancel(t: Tag, stop: () => void): void {
    this._cancels.set(t, stop);
  }
}

/** The most bytes a packet holds, the Canonical ABI's largest list. */
const PKT_CAP = (1 << 28) - 1;

/** The packet `args` write, or none for no args. A packet past the largest
 * is EOVERFLOW. */
function packet(args: Args): U8Array | null {
  if (args === null) return null;
  for (let cap = 1 << 10; ; cap = Math.min(cap * 2, PKT_CAP)) {
    const buf = new Uint8Array(cap);
    const w = new Lower(buf, args[0]);
    args[1](w);
    const n = w.finish();
    if (n >= 0) return buf.subarray(0, n);
    if (cap === PKT_CAP) throw new XputeError(ERRNO_EOVERFLOW);
  }
}
