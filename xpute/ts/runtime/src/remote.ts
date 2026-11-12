// @xpute/runtime/remote.ts

/**
 * A UI on another thread than its core's link, as a page's main thread is to
 * the core's worker, reaches the link over a message port: `serve` on the
 * link's side, `Remote` on the UI's. Packets cross the port as bytes. The UI
 * sends methods to the core's objects and is answered by promise, with the
 * return data. The objects it serves the core are mounted on the Remote
 * before the core boots, and their prefixes go with the core's module, so
 * that `serve` mounts their stand-ins before the core opens them; the link
 * answers their methods 0 at once, since the UI is across the port. The UI
 * never waits, so it has no `call`.
 */

import type { U8Array } from "@xpute/kit/abi/array.ts";
import type { i32 } from "@xpute/kit/abi/word.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { MarshalError } from "@xpute/kit/status/error.ts";
import { type Arg, MAX_PKT_SZ, PacketWriter, TreeReader } from "@xpute/kit/wire/xtp/mod.ts";
import type { Token } from "./abi/link.ts";
import type { Answered, Handle, Link, LinkObject, Method, Opener, Sender } from "./link.ts";

type ToLink = { tag: number; h: Handle; m: Method; pkt: U8Array | null };
type ToUi = { tag: number; result: i32; data: U8Array | null } | { obj: number; m: Method; pkt: U8Array | null };

/** Serves `link` to the UI across `port`, the UI's objects mounted at
 * `prefixes` (`Remote.prefixes`), each by its place there. */
export function serve(port: MessagePort, link: Link, prefixes: readonly (readonly Token[])[]): void {
  const tell = (m: ToUi) => port.postMessage(m);
  prefixes.forEach((prefix, obj) => {
    const o: LinkObject = {
      call(m, _tag, args) {
        tell({ obj, m, pkt: args === null ? null : args.pkt.slice() });
        return 0;
      },
    };
    link.mount(prefix, () => o);
  });
  port.onmessage = ({ data: m }: MessageEvent<ToLink>) => {
    link.send(m.h, m.m, m.pkt ?? [], (result, data) => tell({ tag: m.tag, result, data }));
  };
}

/** The UI's end of a link `serve` serves across a port. */
export class Remote implements Sender {
  /** The prefixes of the objects the UI serves, which `serve` mounts. */
  readonly prefixes: (readonly Token[])[] = [];
  private readonly _port: MessagePort;
  private readonly _answers = new Map<number, (a: Answered) => void>();
  /** The objects the UI serves, each opened once, as it is mounted. */
  private readonly _objects: LinkObject[] = [];
  private _next = 0;

  constructor(port: MessagePort) {
    this._port = port;
    port.onmessage = ({ data: m }: MessageEvent<ToUi>) => {
      if ("tag" in m) {
        const a = this._answers.get(m.tag);
        this._answers.delete(m.tag);
        a?.({ result: m.result, data: m.data });
      } else {
        this._objects[m.obj]?.call(m.m, 0, m.pkt === null ? null : new TreeReader(m.pkt).read_branch());
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

  /** Mounts an object the UI serves, before the core boots. */
  mount(prefix: readonly Token[], open: Opener): void {
    this._objects.push(open(prefix));
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
