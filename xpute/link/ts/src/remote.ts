// @xpute/link/remote.ts

/**
 * A UI on another thread than its guest's link, as a page's main thread is to
 * the guest's worker, reaches the link over a port its runtime hands in
 * (`Port`): its runtime serves the link's side (@xpute/web-runtime remote.ts
 * `serve`, xpute-native-runtime remote.rs), and `Remote` is the UI's. Packets
 * cross the port as bytes, as `ToLink` and `ToUi` lay them. The UI sends
 * methods to the guest's objects and is answered by promise, with the return
 * data. The interfaces it serves the guest are mounted on the Remote under
 * their named handles before the guest boots, and the handles go with the
 * guest's module, so that the runtime mounts their stand-ins before the
 * guest calls them. A method WIT marks `async func` (the bindings' `LATER`)
 * the UI answers later, by its tag (`task_return`), and a cancel of it is
 * told to the UI (`on_cancel`); any other is told to the UI and answered 0 at
 * once, since the UI is across the port. The UI never waits, so it has no
 * `call`.
 *
 * Beside them, a UI may serve the objects at the URIs under a prefix
 * (`mount`), opened across the port and answered later in all their methods,
 * as a device's UI serves the guest's files: the runtime mounts their openers
 * and tells the UI each object's drop. The UI serves as the link does
 * (link.ts `Server`): only how an answer crosses differs. Its open was
 * answered across the port already, so an opener's refusal answers each
 * method of what it would have opened.
 *
 * Beside the link the UI gives a transferable, a canvas, under a handle the
 * worker's side issued, on the same port, so that it lands before what the
 * UI sends after it; it is taken back when the UI drops the handle.
 *
 * A port is one UI's for its whole life.
 */

import type { U8Array } from "@xpute/kit/abi/array.ts";
import type { i32, u32 } from "@xpute/kit/abi/word.ts";
import {
  ERRNO_EINPROGRESS,
  ERRNO_ENOENT,
  ERRNO_EOVERFLOW,
} from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import type { Token } from "./abi/types.ts";
import { Lower } from "./cabi.ts";
import { HDR_SZ } from "./frame.ts";
import {
  type Answered,
  type Args,
  type Handle,
  type LinkObject,
  type Method,
  type Packet,
  type Sender,
  Server,
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

/** The methods of the named interfaces the UI serves that answer later, each
 * `[handle, method]` (the bindings' `LATER`). */
export type Later = readonly (readonly [Handle, Method])[];

/** The prefixes a UI opens objects under, each `[handle, prefix]`, the named
 * handle of the interface whose resources they are (`Remote.prefixes`). */
export type Prefixes = readonly (readonly [Handle, readonly Token[]])[];

/** What the UI sends the link's side: a method of the guest's, a thing
 * given beside the link, or its answer to a method it serves later. */
export type ToLink =
  | { tag: Tag; h: Handle; m: Method; pkt: U8Array | null }
  | { give: Handle; thing: unknown }
  | { answer: Tag; result: i32; data: U8Array | null };

/** What the link's side sends the UI: the answer to one of its methods, a
 * method of an object it serves (to answer by `tag`, or null where it is
 * told), an open of one under a prefix it serves, the guest's cancel of a
 * method it answers later, a drop of an object it opened, and the bytes a
 * frame takes, header and packet, once the guest has laid its link and the
 * UI attached (`Remote.fits`). */
export type ToUi =
  | { tag: Tag; result: i32; data: U8Array | null }
  | { h: Handle; m: Method; tag: Tag | null; pkt: U8Array | null }
  | { open: Handle; uri: readonly Token[] }
  | { cancel: Tag }
  | { drop: Handle }
  | { cap: u32 };

/** The UI's end of a link a runtime serves across a port (@xpute/web-runtime
 * remote.ts `serve`). */
export class Remote
  extends Server<ReturnType<typeof opened>>
  implements Sender
{
  private readonly _port: Port;
  private readonly _answers = new Map<Tag, (a: Answered) => void>();
  private _next = 0;
  /** The bytes a frame takes, as the link's side tells it; null before. */
  private _cap: u32 | null = null;

  constructor(port: Port) {
    super(opened());
    this._port = port;
    port.onmessage = (e) => this._hear(e.data as ToUi);
  }

  /** The named handles of the interfaces the UI serves, which the runtime
   * mounts (@xpute/web-runtime remote.ts `serve`, xpute-native-runtime
   * remote.rs `serve`). */
  get named(): Handle[] {
    return [...this._named.keys()];
  }

  /** The prefixes the UI opens the objects under (`mount`), each with the
   * named handle of the interface whose resources they are. */
  get prefixes(): Prefixes {
    return this._mounts.map(([h, p]) => [h, p]);
  }

  private _hear(m: ToUi): void {
    if ("cap" in m) {
      this._cap = m.cap;
      return;
    }
    if ("result" in m) {
      const a = this._answers.get(m.tag);
      this._answers.delete(m.tag);
      return a?.({ result: m.result, data: m.data });
    }
    if ("open" in m) {
      const open = this._opener(m.uri);
      const refused =
        open === undefined ? ERRNO_ENOENT : this._open(m.open, m.uri, open);
      if (refused !== 0)
        this._objects.put(m.open, { call: () => refused, drop() {} });
      return;
    }
    if ("cancel" in m) return this._cancel(m.cancel);
    if ("drop" in m) return void this._drop(m.drop);
    if (m.tag === null) return void this._call(m.h, m.m, 0, m.pkt);
    const reply = this._call(m.h, m.m, m.tag, m.pkt);
    if (reply === ERRNO_EINPROGRESS) return;
    const { result, data } =
      typeof reply === "number" ? { result: reply, data: null } : reply;
    this.task_return(m.tag, result, data);
  }

  /** Whether the packet `call` sends fits a frame, the call made here and not
   * sent: `call` sends one method at once on the sender it is given, as a
   * binding does, and its packet is judged against the cap the link's side
   * told, as the link's side lays it (link.ts `Side._lay`), so that a call
   * refused whole, EMSGSIZE, is known before it is sent. None fits before
   * the cap is told. */
  fits(call: (link: Sender) => unknown): boolean {
    const cap = this._cap;
    if (cap === null) return false;
    let fit = false;
    const probe: Sender = {
      send(_h, _m, args) {
        if (args === null) fit = true;
        else {
          const w = new Lower(new Uint8Array(cap - HDR_SZ), args[0]);
          args[1](w);
          fit = w.finish() >= 0;
        }
        return Promise.resolve({ result: 0, data: null });
      },
    };
    void Promise.resolve(call(probe)).catch(() => {});
    return fit;
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
    this._port.postMessage({
      answer: t,
      result,
      data: packet(data),
    } satisfies ToLink);
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
