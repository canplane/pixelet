// @xpute/link/remote.ts

/**
 * A UI on another thread than its guest's link, as a page's main thread is to
 * the guest's worker, reaches the link over a port its runtime hands in
 * (`Port`): `serve` on the link's side, `Remote` on the UI's. Packets cross the port as bytes. The UI
 * sends methods to the guest's objects and is answered by promise, with the
 * return data. The interfaces it serves the guest are mounted on the Remote
 * under their named handles before the guest boots, and the handles go with
 * the guest's module, so that `serve` mounts their stand-ins before the guest
 * calls them; the link answers their methods 0 at once, since the UI is across
 * the port. The UI never waits, so it has no `call`.
 *
 * Beside the link the UI gives a transferable, a canvas, under a handle the
 * worker's side issued, on the same port, so that it lands before what the
 * UI sends after it; it is taken back when the UI drops the handle.
 */

import type { U8Array } from "@xpute/kit/abi/array.ts";
import type { i32 } from "@xpute/kit/abi/word.ts";
import { Errno } from "@xpute/kit/status/errno.spec.ts";
import { MarshalError } from "@xpute/kit/status/error.ts";
import { type Arg, MAX_PKT_SZ, PacketWriter, TreeReader } from "@xpute/kit/wire/xtp/mod.ts";
import { type Answered, type Handle, type Link, type LinkObject, type Method, DROP, type Sender } from "./link.ts";

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

type ToLink =
  | { tag: number; h: Handle; m: Method; pkt: U8Array | null }
  | { give: Handle; thing: unknown };
type ToUi =
  | { tag: number; result: i32; data: U8Array | null }
  | { h: Handle; m: Method; pkt: U8Array | null };

/** Serves `link` to the UI across `port`, the interfaces the UI serves
 * mounted under their named handles (`Remote.named`), and what the UI gives
 * beside the link to `taker`. */
export function serve(port: Port, link: Link, named: readonly Handle[], taker: Taker): void {
  const tell = (m: ToUi) => port.postMessage(m);
  for (const h of named) {
    link.mount_named(h, {
      call(m, _tag, args) {
        tell({ h, m, pkt: args === null ? null : args.pkt.slice() });
        return 0;
      },
    });
  }
  port.onmessage = (e) => {
    const m = e.data as ToLink;
    if ("give" in m) return taker.give(m.give, m.thing);
    if (m.m === DROP) taker.take(m.h);
    // A packet past the room is refused whole, EMSGSIZE.
    link.send(m.h, m.m, m.pkt ?? [], (result, data) => tell({ tag: m.tag, result, data }));
  };
}

/** The UI's end of a link `serve` serves across a port. */
export class Remote implements Sender {
  /** The named handles of the interfaces the UI serves, which `serve`
   * mounts. */
  readonly named: Handle[] = [];
  private readonly _port: Port;
  private readonly _answers = new Map<number, (a: Answered) => void>();
  /** The interfaces the UI serves, by their named handles. */
  private readonly _objects = new Map<Handle, LinkObject>();
  private _next = 0;

  constructor(port: Port) {
    this._port = port;
    port.onmessage = (e) => {
      const m = e.data as ToUi;
      if ("tag" in m) {
        const a = this._answers.get(m.tag);
        this._answers.delete(m.tag);
        a?.({ result: m.result, data: m.data });
      } else {
        this._objects.get(m.h)?.call(m.m, 0, m.pkt === null ? null : new TreeReader(m.pkt).read_branch());
      }
    };
  }

  send(h: Handle, m: Method, args: readonly Arg[]): Promise<Answered> {
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
}

/** The packet `args` write, or none for no args. A packet past the largest
 * XTP takes is EOVERFLOW. */
function packet(args: readonly Arg[]): U8Array | null {
  if (args.length === 0) return null;
  for (let cap = 1 << 10; ; cap = Math.min(cap * 2, MAX_PKT_SZ)) {
    const buf = new Uint8Array(cap);
    const w = new PacketWriter().begin(buf, new DataView(buf.buffer), 0, cap, args.length);
    for (const a of args) w.arg(a);
    try {
      return buf.subarray(0, w.finish_or_throw());
    } catch (e) {
      if (!(e instanceof MarshalError) || e.errnum !== Errno.EOVERFLOW || cap === MAX_PKT_SZ) throw e;
    }
  }
}
