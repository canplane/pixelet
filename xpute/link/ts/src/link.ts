// @xpute/link/link.ts

/**
 * The host's link to a guest, alike the guest's (xpute-link `link.rs`,
 * `link/wit/types.wit`). Whatever is called is an object, called by its
 * methods under a handle: an interface's named handle, the hash of its name
 * (`frame.json5` `NAMED`), or a resource's, named by a URI, a tuple of
 * tokens, and opened by the root's `open` to a handle the serving side
 * issues, or answered by a function that returns it. A frame is one method
 * called: its tag the request, unique among the sender's pending ones, its
 * command the handle, the method and a `RES` bit (`frame.ts`), its packet the
 * args in the Canonical ABI's layout (`cabi.ts`). A frame crosses one way
 * through one function, and each crossing returns at once what the method came
 * to: its result, or EINPROGRESS for one that answers later, by a frame marked
 * RES with the request's tag. `call` takes what the crossing returns, for a
 * method that always answers at once, and stops as a bug on EINPROGRESS; `send`
 * hands it to its answer, and an `async func` is sent; `mount_named` serves an
 * interface under its named handle, `add` an object under a handle issued for
 * a function to answer, and `mount` puts the opener of an interface's
 * resources the host serves under a URI's leading tokens, the longest matching
 * winning; `on_cancel` puts what runs if a request it serves is
 * canceled.
 *
 * The link holds these on both ends:
 *
 * - One guest entry at a time: no `link_in` starts before another returns,
 *   and the outside never enters the guest nested.
 * - An answer is never run inside `send`, and a result at once and a later
 *   one take the same path. On the guest, the answers run at the end of the
 *   entry, once its handler has returned, just before `link_in` returns; on
 *   the host, after the work at hand (`queueMicrotask`).
 * - A frame's packet is the receiver's for the crossing alone, and memory its
 *   args lend by (at, len) is the callee's until the answer: the caller keeps
 *   it, as the answer it owns.
 * - The callee answers in the frame the request came in: it lays it again,
 *   marked `RES`, with its result and its return data as the packet. A call's
 *   return data is readable until the next crossing; a send's answer gets a
 *   copy, which it owns.
 */

import type { U8Array } from "@xpute/kit/abi/array.ts";
import type { i32, u32 } from "@xpute/kit/abi/word.ts";
import {
  ERRNO_EBADF,
  ERRNO_EBADMSG,
  ERRNO_EEXIST,
  ERRNO_EINPROGRESS,
  ERRNO_EMSGSIZE,
  ERRNO_ENOENT,
  ERRNO_ENOTRECOVERABLE,
  ERRNO_EPIPE,
} from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import { bug } from "@xpute/kit/status/bug.ts";
import { Lift, Lower } from "./cabi.ts";
import {
  load_Token,
  store_Token,
  type Token,
  TOKEN_ALIGN,
  TOKEN_SZ,
} from "./abi/types.ts";
import { lay, read, HDR_SZ } from "./frame.ts";
import { CANCEL, DROP, NAMED, OPEN, ROOT } from "./frame.spec.ts";
import { type Handle, Table } from "./resource.ts";

export type Tag = u32;
export type Method = u32;
export type { Handle } from "./resource.ts";

/** The interfaces a guest's world imports, each `[name, handle]` (the
 * bindings' `IMPORTS`). */
export type Imports = readonly (readonly [string, Handle])[];

export { CANCEL, DROP, NAMED, OPEN, ROOT };

/** An object a side serves: given a method, the request's tag, for a
 * later answer (`task_return`), and the args, it answers; and it is told its
 * drop once the other side drops its handle, as a Rust object is dropped
 * (xpute-link `link.rs` `Object`). */
export interface LinkObject {
  call(m: Method, tag: Tag, args: U8Array | null): i32 | Reply;
  drop(): void;
}

/** What opens the object a URI names, given the handle it is issued. */
export type Opener = (uri: readonly Token[], h: Handle) => LinkObject;

/** A method's args, or its return data: its fixed part's size, and what
 * writes it (`Lower`). Null is no packet (xpute-link link.rs `Args`). */
export type Args = readonly [u32, (w: Lower) => void] | null;

/** What a method answers: its result, a status, and its return data, its
 * value, written once its args are read. */
export interface Reply {
  result: i32;
  data: Args;
}

/** What hears a sent method's answer: its result, and a copy of its return
 * data, which it owns. */
export type Answer = (result: i32, data: U8Array | null) => void;

/** A call's answer: its result, and its return data, readable until the next
 * crossing. */
export interface Answered {
  result: i32;
  data: U8Array | null;
}

/** A frame's packet: the args, written as the frame is laid, or a packet
 * written already, as one that crossed a port. */
export type Packet = Args | U8Array;

/** What sends a method to the other side and is answered by promise: a UI's
 * `Remote`, or a link through `wait`. */
export interface Sender {
  send(h: Handle, m: Method, args: Args): Promise<Answered>;
}

/** A reference to an object on the other side: the link it is reached
 * over, and the handle that side issued. */
export interface Ref {
  readonly link: Sender;
  readonly handle: Handle;
}

/** The URI of names alone, as a resource's (package, resource). */
export function uri(...names: string[]): Token[] {
  return names.map((val) => ({ tag: "str", val }));
}

/** Writes `uri` at `at` as a `list<token>`, as `ROOT` `OPEN`'s args hold it
 * (`types.wit`). */
export function store_uri(w: Lower, at: number, uri: readonly Token[]): void {
  const b = w.list(at, uri.length, TOKEN_SZ, TOKEN_ALIGN);
  if (b !== null) uri.forEach((t, k) => store_Token(w, b + k * TOKEN_SZ, t));
}

/** The URI the `list<token>` at `at` holds (`store_uri`). */
export function load_uri(r: Lift, at: number): Token[] {
  return r.map(at, TOKEN_SZ, TOKEN_ALIGN, (off) => load_Token(r, off));
}

/** Opens the object `uri` names on the other side, and answers its handle. */
export async function open(
  link: Sender,
  uri: readonly Token[],
): Promise<Handle> {
  const a = await link.send(ROOT, OPEN, [8, (w) => store_uri(w, 0, uri)]);
  if (a.result !== 0) throw new XputeError(a.result);
  if (a.data === null) throw new XputeError(ERRNO_EBADMSG);
  return new Lift(a.data).u32(0);
}

/** Lets the object `h` names on the other side go. */
export function drop(link: Sender, h: Handle): void {
  void link.send(h, DROP, null);
}

/** A function's `result` error case, as jco's bindings carry it: its caller
 * is thrown it, and its implementation throws it. `payload` is the case's
 * value. */
export class ComponentError<P = unknown> extends Error {
  readonly payload: P;

  constructor(payload: P) {
    super(`component error: ${String(payload)}`);
    this.payload = payload;
  }
}

/** What a failure of the serving side's own is handed (`set_hook`). */
let hook: ((e: unknown) => void) | null = null;

/** Installs what a failure of the serving side's own, a throw out of its
 * answer to the guest's import (`imports`), is handed, as Rust's panic hook
 * is (@xpute/web-runtime worker.ts `hook`). Only the first call takes
 * effect. */
export function set_hook(h: (e: unknown) => void): void {
  hook ??= h;
}

/** The errno a method refuses with: an XputeError's. Anything else, a `Bug`
 * or any other throw, is a failure of the serving side's own, thrown on: out
 * of a method answered at once, to what called it (on the host, the guest's
 * import, `imports`; on the UI, its port's message handler, `Remote`); out of
 * one answered later, on either side, as a rejection no one handles. */
export function refusal_of(e: unknown): i32 {
  if (e instanceof XputeError) return e.errnum;
  throw e;
}

/** What the host holds of its guest: the guest's memory, its three exports
 * (`link/wit/types.wit`), and its platform's report of a trap (`trap`). An
 * export the guest traps in traps the link (`Side.trap`) and answers on,
 * throwing nothing: `link_in` EPIPE, as a trapped guest's request is
 * answered. */
export interface Guest {
  readonly mem: ArrayBuffer;
  /** Runs `link_init`, which answers where the link's layout lies, the inbox
   * and a frame's cap. */
  init(): u32;
  /** Runs `link_boot`, the guest's first entry, which boots it. */
  boot(): void;
  /** Runs `link_in` on the frame laid in the inbox. */
  in(): i32;
  /** Tells the guest's platform the guest trapped, with the error `e`
   * (`Side.trap`). */
  trap(e: unknown): void;
}

/** What a side serves, alike on the host's link and on a UI across a port
 * (`Side`, `Remote`), as xpute-link `link.rs` `Side` serves beside its frames,
 * Rust naming this part apart nowhere: its interfaces by their named
 * handles, the objects under issued handles in a table (`Table`: a `Side`'s,
 * which issues them, or a UI's, under those its link's side issued), the
 * openers of an interface's resources by the URI's leading tokens, the
 * longest matching winning, and how
 * each request it answers later stops if its caller cancels it. Only how an
 * answer crosses differs (`_answer`). */
export abstract class Server<
  O extends Pick<Table<LinkObject>, "get" | "put" | "remove">,
> {
  /** The openers, each with the named handle of the interface whose
   * resources it opens, by the URI's leading tokens. */
  protected readonly _mounts: [Handle, readonly Token[], Opener][] = [];
  /** The interfaces this side serves, by their named handles (`NAMED`). */
  protected readonly _named = new Map<Handle, LinkObject>();
  /** The objects this side serves, by the handles issued for them. */
  protected readonly _objects: O;
  /** How each request this side serves stops if its caller cancels it, by
   * tag, until it answers. */
  private readonly _cancels = new Map<Tag, () => void>();

  constructor(objects: O) {
    this._objects = objects;
  }

  /** Puts `open` as the opener of the URIs that start with `prefix`, whose
   * objects are the resources of the interface of the named handle `h`. */
  mount(h: Handle, prefix: readonly Token[], open: Opener): void {
    this._mounts.push([h, prefix, open]);
  }

  /** Whether this side serves the interface of the named handle `h`: under
   * that handle, or by an opener of its resources. */
  serves(h: Handle): boolean {
    return this._named.has(h) || this._mounts.some(([i]) => i === h);
  }

  /** Serves `o` under its interface's named handle `h`, which the other
   * side calls with no open. */
  mount_named(h: Handle, o: LinkObject): void {
    if (this._named.has(h)) bug(ERRNO_EEXIST);
    this._named.set(h, o);
  }

  /** Puts `stop` as what runs if the caller cancels the request `t` this
   * side serves (`frame.json5` `CANCEL`): a method that can stop then answers
   * ECANCELED, and one that can't runs on. */
  on_cancel(t: Tag, stop: () => void): void {
    this._cancels.set(t, stop);
  }

  /** Answers the request `t`, whose method answered EINPROGRESS, with
   * `result` and its return data, as the component model's `task.return`. A
   * later answer is found by its tag alone. */
  task_return(t: Tag, result: i32, data: Packet = null): void {
    this._cancels.delete(t);
    this._answer(t, result, data);
  }

  /** Carries the answer to the request `t` to the other side. */
  protected abstract _answer(t: Tag, result: i32, data: Packet): void;

  /** Runs `m` of `h`, an interface's or an object's method. A named handle
   * no interface is mounted under is no object here; an issued one no longer
   * held is a bad handle. */
  protected _call(
    h: Handle,
    m: Method,
    tag: Tag,
    args: U8Array | null,
  ): i32 | Reply {
    if ((h & NAMED) !== 0) {
      const o = this._named.get(h);
      return o === undefined ? ERRNO_ENOENT : o.call(m, tag, args);
    }
    const o = this._objects.get(h);
    return o === undefined ? ERRNO_EBADF : o.call(m, tag, args);
  }

  /** The opener of `uri`'s longest matching mount, if one matches. */
  protected _opener(uri: readonly Token[]): Opener | undefined {
    let best: [Handle, readonly Token[], Opener] | undefined;
    for (const m of this._mounts) {
      const [, p] = m;
      const leads =
        p.length <= uri.length &&
        p.every((t, k) => t.tag === uri[k]!.tag && t.val === uri[k]!.val);
      if (leads && (best === undefined || p.length > best[1].length)) best = m;
    }
    return best?.[2];
  }

  /** Opens the object at `uri` under `h` by `open`: 0, or the opener's
   * refusal, with nothing put under `h`. */
  protected _open(h: Handle, uri: readonly Token[], open: Opener): i32 {
    try {
      this._objects.put(h, open(uri, h));
      return 0;
    } catch (e) {
      return refusal_of(e);
    }
  }

  /** Runs the stop of the request `t`, which its caller canceled. */
  protected _cancel(t: Tag): void {
    const stop = this._cancels.get(t);
    this._cancels.delete(t);
    stop?.();
  }

  /** Drops the object under `h` and tells it its drop: 0, or EBADF where
   * none is held. */
  protected _drop(h: Handle): i32 {
    const o = this._objects.remove(h);
    if (o === undefined) return ERRNO_EBADF;
    o.drop();
    return 0;
  }
}

/** The host's side of the link, as xpute-link `link.rs` `Side` is either
 * side: what it serves (`Server`), and the frames it lays in the guest's
 * inbox and takes from its outbox. */
export class Side extends Server<Table<LinkObject>> {
  /** The answers of the sent requests that answer later, by tag. */
  private readonly _pending = new Map<Tag, Answer>();
  private _next: Tag = 0;
  private readonly _guest: Guest;
  /** Whether the guest is trapped (`trap`). */
  private _trapped = false;
  private _inbox = 0;
  private _cap = 0;

  /** The guest's import, which a module takes under `env`
   * (xpute-native-runtime `link.rs` `link_out`). A throw out of the host
   * while it answers is a failure of its own: it is handed to the hook, the
   * guest is trapped, and its call is answered ENOTRECOVERABLE, so nothing
   * unwinds through it. Once trapped, a frame is not served: EPIPE. */
  static imports(link: () => Side) {
    return {
      link_out: (at: u32): i32 => {
        const l = link();
        if (l._trapped) return ERRNO_EPIPE;
        try {
          return l._recv(at);
        } catch (e) {
          hook?.(e);
          l.trap(e);
          return ERRNO_ENOTRECOVERABLE;
        }
      },
    };
  }

  /** Stops the guest for good, where it has not trapped already: it is
   * entered no more, a frame sent to it or laid by it answering EPIPE, and
   * its platform is told (`Guest.trap`), as xpute-native-runtime `link.rs`
   * `trap`. Each request this side sent that is still pending is answered
   * EPIPE, once, as xpute-link `link.rs` `Side::trap` answers it; none is
   * pending after, so a late answer to one finds nothing to answer. */
  trap(e: unknown): void {
    if (this._trapped) return;
    this._trapped = true;
    const pending = [...this._pending.values()];
    this._pending.clear();
    for (const a of pending) queueMicrotask(() => a(ERRNO_EPIPE, null));
    this._guest.trap(e);
  }

  /** Whether the guest is trapped (`trap`). */
  trapped(): boolean {
    return this._trapped;
  }

  /** The bytes a frame takes, header and packet, which the guest laid its
   * link with (`init`); 0 before. */
  get cap(): u32 {
    return this._cap;
  }

  constructor(guest: Guest) {
    super(new Table<LinkObject>());
    this._guest = guest;
  }

  /** Enters the guest first, once the host's own objects are mounted: the
   * guest lays its link, the host reads its layout, and then the guest
   * boots, and may open them. Where an interface of `imports` is served by
   * nothing mounted, under its named handle or by an opener of its
   * resources (`serves`), the link traps, naming each, and the guest is
   * never entered. False where the link trapped meanwhile, the
   * guest's trap or a failure of the host's (xpute-native-runtime
   * `link.rs` `init`). */
  init(imports: Imports): boolean {
    const unserved = imports
      .filter(([, h]) => !this.serves(h))
      .map(([name]) => name);
    if (unserved.length > 0) {
      this.trap(`no server mounts ${unserved.join(", ")}`);
      return false;
    }
    const at = this._guest.init();
    if (this._trapped) return false;
    const layout = new DataView(this._guest.mem, at, 8);
    this._inbox = layout.getUint32(0, true);
    this._cap = layout.getUint32(4, true);
    this._guest.boot();
    return !this._trapped;
  }

  call(h: Handle, m: Method, args: Packet = null): Answered {
    const answered = this._out(this._tag(), h, m, false, 0, args);
    if (answered.result === ERRNO_EINPROGRESS) bug(ERRNO_EINPROGRESS);
    return answered;
  }

  /** Sends `m` of `h`, its answer pending from before the crossing, so a
   * later answer the callee gives inside it finds its request. */
  send(h: Handle, m: Method, args: Packet = null, answer?: Answer): void {
    const t = this._tag();
    if (answer !== undefined) this._pending.set(t, answer);
    let out: Answered;
    try {
      out = this._out(t, h, m, false, 0, args);
    } catch (e) {
      this._pending.delete(t);
      throw e;
    }
    const { result, data } = out;
    if (
      result === ERRNO_EINPROGRESS ||
      !this._pending.delete(t) ||
      answer === undefined
    )
      return;
    const copy = data && data.slice();
    queueMicrotask(() => answer(result, copy));
  }

  /** Serves `o` under a handle issued for it, which a method answers for the
   * other side to call `o` by: a resource a function returns. */
  add(o: LinkObject): Handle {
    return this._objects.add(o);
  }

  /** Lays the answer to the request `t` in the guest's inbox. */
  protected _answer(t: Tag, result: i32, data: Packet): void {
    this._out(t, ROOT, 0, true, result, data);
  }

  /** The guest's memory, as it stands now: it grows, so it is read anew. */
  get mem(): ArrayBuffer {
    return this._guest.mem;
  }

  /** The `len` bytes at `at` in the guest's memory. */
  bytes(at: u32, len: u32): U8Array {
    return new Uint8Array(this._guest.mem, at, len);
  }

  /** The frame the guest laid at `at`: a later answer, to its request, or a
   * method, to its object. */
  private _recv(at: u32): i32 {
    const f = read(new DataView(this._guest.mem), at);
    if (f.res) {
      const a = this._pending.get(f.tag);
      this._pending.delete(f.tag);
      const data =
        f.pkt_len > 0 ? this.bytes(at + HDR_SZ, f.pkt_len).slice() : null;
      if (a !== undefined) queueMicrotask(() => a(f.result, data));
      return 0;
    }
    const args = this._args(at, f.pkt_len);
    const reply =
      typeof args === "number"
        ? args
        : this._serve(f.handle, f.method, f.tag, args);
    // The args are read: the answer goes in the frame they came in.
    const { result, data } =
      typeof reply === "number" ? { result: reply, data: null } : reply;
    // An answer past the cap is refused as a later one is (`_out`).
    if (this._lay(at, f.tag, f.handle, f.method, true, result, data) !== 0) {
      this._lay(at, f.tag, f.handle, f.method, true, ERRNO_EMSGSIZE, null);
      return ERRNO_EMSGSIZE;
    }
    return result;
  }

  /** Runs `m` of `h`: the root's open or cancel, a drop, or an object's
   * method. */
  private _serve(
    h: Handle,
    m: Method,
    tag: Tag,
    args: U8Array | null,
  ): i32 | Reply {
    if (h === ROOT && m === OPEN) return this._opened(args);
    if (h === ROOT && m === CANCEL) {
      try {
        if (args === null) return ERRNO_EBADMSG;
        this._cancel(new Lift(args).u32(0));
        return 0;
      } catch (e) {
        return refusal_of(e);
      }
    }
    if (m === DROP) return this._drop(h);
    return this._call(h, m, tag, args);
  }

  /** Opens the object the args' URI names, by the opener of its longest
   * matching mount, under a handle issued for it, and answers the handle; or
   * ENOENT where no mount matches, or the opener's refusal, the handle given
   * back. */
  private _opened(args: U8Array | null): i32 | Reply {
    if (args === null) return ERRNO_EBADMSG;
    let uri: Token[];
    try {
      uri = load_uri(new Lift(args), 0);
    } catch (e) {
      return refusal_of(e);
    }
    const open = this._opener(uri);
    if (open === undefined) return ERRNO_ENOENT;
    const h = this._objects.issue();
    const refused = this._open(h, uri, open);
    if (refused !== 0) {
      this._objects.remove(h);
      return refused;
    }
    return { result: 0, data: [4, (w) => w.u32(0, h)] };
  }

  /** The next request's tag: a counter that wraps, past every tag still
   * pending, so a late answer, which comes only for a pending tag, finds its
   * own request. */
  private _tag(): Tag {
    let t = this._next;
    while (this._pending.has(t)) t = (t + 1) >>> 0;
    this._next = (t + 1) >>> 0;
    return t;
  }

  /** The args of the frame at `at`, or EBADMSG for a packet past the cap. */
  private _args(at: u32, pkt_len: u32): U8Array | null | i32 {
    if (pkt_len === 0) return null;
    if (HDR_SZ + pkt_len > this._cap) return ERRNO_EBADMSG;
    return this.bytes(at + HDR_SZ, pkt_len);
  }

  /** Lays a frame at `at` in the guest's memory, with the packet `args`: 0,
   * or EMSGSIZE, laying nothing, where the packet is past the cap
   * (xpute-link link.rs `Side::lay`). */
  private _lay(
    at: u32,
    tag: Tag,
    handle: Handle,
    method: Method,
    res: boolean,
    result: i32,
    args: Packet,
  ): i32 {
    const bytes = new Uint8Array(this._guest.mem, at, this._cap);
    const view = new DataView(this._guest.mem, at, this._cap);
    let pkt_len = 0;
    if (args instanceof Uint8Array) {
      if (HDR_SZ + args.length > this._cap) return ERRNO_EMSGSIZE;
      bytes.set(args, HDR_SZ);
      pkt_len = args.length;
    } else if (args !== null) {
      const w = new Lower(bytes.subarray(HDR_SZ), args[0]);
      args[1](w);
      pkt_len = w.finish();
      if (pkt_len < 0) return ERRNO_EMSGSIZE;
    }
    lay(view, 0, { tag, res, handle, method, result, pkt_len });
    return 0;
  }

  /** Lays a frame in the guest's inbox and crosses with it, and answers what
   * the guest laid there: its result, and its return data. A packet past the
   * cap is refused: a packet written elsewhere, as one from across a port,
   * is its sender's fault, answered EMSGSIZE; a later answer's is answered
   * EMSGSIZE with no data, so its request still hears; args this side
   * writes past it are a bug (xpute-link `link.rs` `Side::out`). A trapped
   * guest is sent nothing: EPIPE, its layout perhaps never read. */
  private _out(
    tag: Tag,
    h: Handle,
    m: Method,
    res: boolean,
    result: i32,
    args: Packet,
  ): Answered {
    if (this._trapped) return { result: ERRNO_EPIPE, data: null };
    if (this._lay(this._inbox, tag, h, m, res, result, args) !== 0) {
      if (!res) {
        if (!(args instanceof Uint8Array)) bug(ERRNO_EMSGSIZE);
        return { result: ERRNO_EMSGSIZE, data: null };
      }
      this._lay(this._inbox, tag, h, m, res, ERRNO_EMSGSIZE, null);
    }
    const answered = this._guest.in();
    const f = read(new DataView(this._guest.mem), this._inbox);
    const data =
      f.res && !res && f.pkt_len > 0
        ? this.bytes(this._inbox + HDR_SZ, f.pkt_len)
        : null;
    return { result: answered, data };
  }
}
