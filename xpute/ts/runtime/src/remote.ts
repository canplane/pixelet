// @xpute/runtime/remote.ts

/**
 * A UI on another thread than its core's link, as a page's main thread is to
 * the core's worker, reaches the link over a message port: `serve` on the
 * link's side, `Remote` on the UI's. Packets cross the port as bytes. The UI
 * sends methods to the core's objects and is answered by promise, with the
 * return data. The openers of the objects it serves the core are mounted on
 * the Remote before the core boots, and their prefixes go with the core's
 * module, so that `serve` mounts their stand-ins before the core opens them;
 * each open of a stand-in opens the UI's object under the handle the link
 * issued, and the link answers its methods 0 at once, since the UI is across
 * the port. The UI never waits, so it has no `call`.
 *
 * Beside the link the UI gives a transferable, a canvas, under a handle the
 * worker's side issued, on the same port, so that it lands before what the
 * UI sends after it; it is taken back when the UI releases the handle.
 */

import type { U8Array } from "@xpute/kit/abi/array.ts";
import type { i32 } from "@xpute/kit/abi/word.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { MarshalError } from "@xpute/kit/status/error.ts";
import { type Arg, MAX_PKT_SZ, PacketWriter, TreeReader } from "@xpute/kit/wire/xtp/mod.ts";
import type { Token } from "./abi/link.ts";
import { type Answered, type Handle, type Link, type LinkObject, type Method, type Opener, RELEASE, type Sender } from "./link.ts";

/** What takes the things the UI gives beside the link, by the handle it
 * gives each under, and hears the handle released. */
export interface Taker {
  give(h: Handle, thing: Transferable): void;
  take(h: Handle): void;
}

type ToLink =
  | { tag: number; h: Handle; m: Method; pkt: U8Array | null }
  | { give: Handle; thing: Transferable };
type ToUi =
  | { tag: number; result: i32; data: U8Array | null }
  | { open: number; uri: Token[]; h: Handle }
  | { h: Handle; m: Method; pkt: U8Array | null };

/** Serves `link` to the UI across `port`, the UI's objects mounted at
 * `prefixes` (`Remote.prefixes`), each by its place there, and what the UI
 * gives beside the link to `taker`. */
export function serve(
  port: MessagePort,
  link: Link,
  prefixes: readonly (readonly Token[])[],
  taker: Taker,
): void {
  const tell = (m: ToUi) => port.postMessage(m);
  prefixes.forEach((prefix, open) => {
    link.mount(prefix, (uri, h): LinkObject => {
      tell({ open, uri: [...uri], h });
      return {
        call(m, _tag, args) {
          tell({ h, m, pkt: args === null ? null : args.pkt.slice() });
          return 0;
        },
      };
    });
  });
  port.onmessage = ({ data: m }: MessageEvent<ToLink>) => {
    if ("give" in m) return taker.give(m.give, m.thing);
    if (m.m === RELEASE) taker.take(m.h);
    try {
      link.send(m.h, m.m, m.pkt ?? [], (result, data) => tell({ tag: m.tag, result, data }));
    } catch (e) {
      // A packet past the room is refused whole.
      if (!(e instanceof MarshalError)) throw e;
      tell({ tag: m.tag, result: e.errnum, data: null });
    }
  };
}

/** The UI's end of a link `serve` serves across a port. */
export class Remote implements Sender {
  /** The prefixes of the objects the UI serves, which `serve` mounts. */
  readonly prefixes: (readonly Token[])[] = [];
  private readonly _port: MessagePort;
  private readonly _answers = new Map<number, (a: Answered) => void>();
  /** The openers of the objects the UI serves, by their place in
   * `prefixes`, and the objects opened, by the handle each was issued. */
  private readonly _opens: Opener[] = [];
  private readonly _objects = new Map<Handle, LinkObject>();
  private _next = 0;

  constructor(port: MessagePort) {
    this._port = port;
    port.onmessage = ({ data: m }: MessageEvent<ToUi>) => {
      if ("tag" in m) {
        const a = this._answers.get(m.tag);
        this._answers.delete(m.tag);
        a?.({ result: m.result, data: m.data });
      } else if ("open" in m) {
        this._objects.set(m.h, this._opens[m.open]!(m.uri, m.h));
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
   * issued, until the UI releases `h`. */
  give(h: Handle, thing: Transferable): void {
    this._port.postMessage({ give: h, thing } satisfies ToLink, [thing]);
  }

  /** Mounts the opener of an object the UI serves, before the core boots. */
  mount(prefix: readonly Token[], open: Opener): void {
    this._opens.push(open);
    this.prefixes.push(prefix);
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
