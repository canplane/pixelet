// @xpute/link/link.ts

/**
 * An end of the link, alike the guest's (xpute-link `link.rs`,
 * `link/wit/types.wit`): what every end runs alike (`End`), over the
 * transport that takes its frames to the other end, the guest's boxes for the
 * host's (`Side`). Whatever is called is an
 * object, called by its methods under a handle: an interface's named handle, the hash of its name
 * (`frame.json5` `NAMED`), or a resource's, named by a URI, a tuple of
 * tokens, and opened by the root's `open` to a handle the implementing side
 * issues, or answered by a function that returns it. A frame is one method
 * called: its tag the request, unique among the sender's pending ones, its
 * command the handle, the method and a `RES` bit (`frame.ts`), its packet the
 * args in the Canonical ABI's layout (`cabi.ts`). A frame crosses one way
 * through one function, and each crossing returns at once what the method came
 * to: its result, or EINPROGRESS for one that answers later, by a frame marked
 * RES with the request's tag. `call` takes what the crossing returns, for a
 * method that always answers at once, and stops as a bug on EINPROGRESS; `send`
 * hands it to its answer, and an `async func` is sent; `mount_named` mounts an
 * interface's implementation under its named handle, `add` an object under a
 * handle issued for a function to answer, and `mount` puts the opener of an
 * interface's resources the host implements under a URI's leading tokens, the
 * longest matching winning; `on_cancel` puts what runs if a request it
 * answers is canceled.
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
import { join64, type i32, type u32, type u64 } from "@xpute/kit/abi/word.ts";
import {
  ERRNO_EBADF,
  ERRNO_EBADMSG,
  ERRNO_EEXIST,
  ERRNO_EINPROGRESS,
  ERRNO_EMSGSIZE,
  ERRNO_ENOENT,
  ERRNO_EOVERFLOW,
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
import { CANCEL, DROP, LAYOUT, NAMED, OPEN, ROOT } from "./frame.spec.ts";
import { type Handle, Table } from "./resource.ts";

export type Tag = u32;
export type Method = u32;
export type { Handle } from "./resource.ts";

/** The interfaces a guest's world imports, each `[name, handle]` (the
 * bindings' `IMPORTS`). */
export type Imports = readonly (readonly [string, Handle])[];

export { CANCEL, DROP, NAMED, OPEN, ROOT };

/** An object a side implements: given a method, the request's tag, for a
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
 * written already, as one a runtime carried from another thread. */
export type Packet = Args | U8Array;

/** The most bytes a packet holds, the Canonical ABI's largest list. */
const PKT_CAP = (1 << 28) - 1;

/** The bytes of the packet `args`: those `args` write, those it is, or none
 * for no args. A packet past the largest is EOVERFLOW. */
export function pkt_of(args: Packet): U8Array | null {
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

/** What the generated bindings call by: it sends a method to the object
 * under a handle and is answered by promise. An end is one through `wait`
 * (`sender`). */
export interface Sender {
  send(h: Handle, m: Method, args: Args): Promise<Answered>;
}

/** What the generated bindings implement by, the callee's end: it mounts an
 * interface's implementation under its named handle (`mount_named`), puts the
 * opener of an interface's resources under a URI's leading tokens (`mount`),
 * answers a request later by its tag (`task_return`), and puts what runs if
 * the caller cancels a request (`on_cancel`). An end (`End`) is one. */
export interface Callee {
  mount_named(h: Handle, o: LinkObject): void;
  mount(h: Handle, prefix: readonly Token[], open: Opener): void;
  task_return(t: Tag, result: i32, data?: Packet): void;
  on_cancel(t: Tag, stop: () => void): void;
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

/** The errno a method refuses with: an XputeError's. Anything else, a `XputeBug`
 * or any other throw, is a failure of the callee's own, thrown on: out
 * of a method answered at once, to what called it (on the host, the guest's
 * import, `Side.recv`; on a UI, its runtime's message handler); out of
 * one answered later, on either side, as a rejection no one handles. */
export function refusal_of(e: unknown): i32 {
  if (e instanceof XputeError) return e.errnum;
  throw e;
}

/** What reads the guest's memory in place, where what the guest lends lies:
 * the host's end (`Side`), or a device's UI end, in the guest's process. */
export interface Memory {
  /** The `len` bytes the guest lent at `at`, its address
   * (link/wit/types.wit `lent`). */
  bytes(at: u64, len: u32): U8Array;
}

/** One end of the link, the part both ends run alike, as xpute-link `link.rs`
 * `Side` runs it beside its frames: what it implements, its interfaces by
 * their named handles, the objects in a table (`Table`) under handles, the
 * openers of an interface's resources by the URI's leading tokens, the
 * longest matching winning, how each request it answers later stops if its
 * caller cancels it, and the answers of what it sent, by tag. How a frame
 * crosses to the other end is its transport's (`_out`), which an end over it
 * implements, and the transport hands the end each frame from the other end
 * (`input`): the guest's boxes and the crossing into it (`Side`), or a
 * runtime's transport to a UI on another thread. */
export abstract class End implements Callee {
  /** The openers, each with the named handle of the interface whose
   * resources it opens, by the URI's leading tokens. */
  private readonly _mounts: [Handle, readonly Token[], Opener][] = [];
  /** The interfaces this end implements, by their named handles (`NAMED`). */
  private readonly _named = new Map<Handle, LinkObject>();
  /** The objects this end implements, by their handles: those it issued, or,
   * for an object the other end opened under a handle it issued, that
   * handle (`open_at`). */
  private readonly _objects = new Table<LinkObject>();
  /** How each request this end answers stops if its caller cancels it, by
   * tag, until it answers. */
  private readonly _cancels = new Map<Tag, () => void>();
  /** The answers of the sent requests that answer later, by tag. */
  private readonly _pending = new Map<Tag, Answer>();
  private _next: Tag = 0;
  /** Whether the link is trapped (`trap`). */
  private _trapped = false;

  /** Carries a frame to the other end, a request or a later answer, with
   * the packet `pkt`, and answers what the method came to there: its result
   * and its return data at once, or EINPROGRESS for one answered later, by
   * a frame the transport hands this end (`input`). */
  protected abstract _out(
    tag: Tag,
    h: Handle,
    m: Method,
    res: boolean,
    result: i32,
    pkt: Packet,
  ): Answered;

  /** Stops the link for good: nothing is carried, a frame sent answering
   * EPIPE, and each request this end sent that is still pending is answered
   * EPIPE, once, as xpute-link `link.rs` `Side::trap` answers it; none is
   * pending after, so a late answer to one finds nothing to answer. The
   * runtime tells its platform (@xpute/runtime-web worker.ts `trap`). */
  trap(): void {
    this._trapped = true;
    const pending = [...this._pending.values()];
    this._pending.clear();
    for (const a of pending) queueMicrotask(() => a(ERRNO_EPIPE, null));
  }

  /** Whether the link is trapped (`trap`). */
  trapped(): boolean {
    return this._trapped;
  }

  /** Puts `open` as the opener of the URIs that start with `prefix`, whose
   * objects are the resources of the interface of the named handle `h`. */
  mount(h: Handle, prefix: readonly Token[], open: Opener): void {
    this._mounts.push([h, prefix, open]);
  }

  /** The named handles of the interfaces this end implements. */
  named(): Handle[] {
    return [...this._named.keys()];
  }

  /** The prefixes this end opens objects under, in the order they were
   * mounted, each with the named handle of the interface whose resources
   * they are. */
  prefixes(): (readonly [Handle, readonly Token[]])[] {
    return this._mounts.map(([h, p]) => [h, p]);
  }

  /** Whether this end implements the interface of the named handle `h`:
   * under that handle, or by an opener of its resources. */
  implements(h: Handle): boolean {
    return this._named.has(h) || this._mounts.some(([i]) => i === h);
  }

  /** Mounts `o` under its interface's named handle `h`, which the other
   * end calls with no open. */
  mount_named(h: Handle, o: LinkObject): void {
    if (this._named.has(h)) bug(ERRNO_EEXIST);
    this._named.set(h, o);
  }

  /** Adds `o` under a handle issued for it, which a method answers for the
   * other end to call `o` by: a resource a function returns. */
  add(o: LinkObject): Handle {
    return this._objects.add(o);
  }

  /** Opens the object at `uri` under `h`, a handle the other end issued, by
   * the opener of this end's `k`th mount (`prefixes`): the other end
   * answered its open at once and issued `h`, and this end opens the object
   * later, as a runtime carries the open to a UI on another thread. An
   * opener that refuses answers each method of what it would have opened
   * with the refusal. */
  open_at(k: number, h: Handle, uri: readonly Token[]): void {
    const [, , open] = this._mounts[k] ?? bug(ERRNO_EBADF);
    let o: LinkObject;
    try {
      o = open(uri, h);
    } catch (e) {
      const refused = refusal_of(e);
      o = { call: () => refused, drop() {} };
    }
    this._objects.put(h, o);
  }

  /** Puts `stop` as what runs if the caller cancels the request `t` this
   * end answers (`frame.json5` `CANCEL`): a method that can stop then answers
   * ECANCELED, and one that can't runs on. */
  on_cancel(t: Tag, stop: () => void): void {
    this._cancels.set(t, stop);
  }

  /** Answers the request `t`, whose method answered EINPROGRESS, with
   * `result` and its return data, as the component model's `task.return`. A
   * later answer is found by its tag alone. */
  task_return(t: Tag, result: i32, data: Packet = null): void {
    this._cancels.delete(t);
    this._carry(t, ROOT, 0, true, result, data);
  }

  /** Calls `m` of `h`, which answers at once: its result, and its return
   * data, readable until the next crossing. A later answer is a bug. */
  call(h: Handle, m: Method, args: Packet = null): Answered {
    const answered = this._carry(this._tag(), h, m, false, 0, args);
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
      out = this._carry(t, h, m, false, 0, args);
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

  /** The frame the transport brought from the other end: a later answer, to
   * this end's request, heard after the work at hand with a copy of its
   * data, answering 0; or a method, to an object this end implements, answering
   * its reply, which the transport carries back where it is not EINPROGRESS:
   * the root's open or cancel, a drop, or an object's method. `args` are the
   * receiver's for the crossing alone. */
  input(
    tag: Tag,
    h: Handle,
    m: Method,
    res: boolean,
    result: i32,
    args: U8Array | null,
  ): i32 | Reply {
    if (res) {
      const a = this._pending.get(tag);
      this._pending.delete(tag);
      const data = args && args.slice();
      if (a !== undefined) queueMicrotask(() => a(result, data));
      return 0;
    }
    if (h === ROOT && m === OPEN) return this._opened(args);
    if (h === ROOT && m === CANCEL) {
      try {
        if (args === null) return ERRNO_EBADMSG;
        const t = new Lift(args).u32(0);
        const stop = this._cancels.get(t);
        this._cancels.delete(t);
        stop?.();
        return 0;
      } catch (e) {
        return refusal_of(e);
      }
    }
    if (m === DROP) {
      const o = this._objects.remove(h);
      if (o === undefined) return ERRNO_EBADF;
      o.drop();
      return 0;
    }
    // A named handle no interface is mounted under is no object here; an
    // issued one no longer held is a bad handle.
    if ((h & NAMED) !== 0) {
      const o = this._named.get(h);
      return o === undefined ? ERRNO_ENOENT : o.call(m, tag, args);
    }
    const o = this._objects.get(h);
    return o === undefined ? ERRNO_EBADF : o.call(m, tag, args);
  }

  /** The opener of `uri`'s longest matching mount, if one matches. */
  private _opener(uri: readonly Token[]): Opener | undefined {
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
    try {
      this._objects.put(h, open(uri, h));
    } catch (e) {
      this._objects.remove(h);
      return refusal_of(e);
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

  /** Carries a frame by the transport; a trapped link carries nothing:
   * EPIPE, its transport perhaps never laid. */
  private _carry(
    tag: Tag,
    h: Handle,
    m: Method,
    res: boolean,
    result: i32,
    pkt: Packet,
  ): Answered {
    if (this._trapped) return { result: ERRNO_EPIPE, data: null };
    return this._out(tag, h, m, res, result, pkt);
  }
}

/** What the host's side holds of the guest: its memory, as it stands now,
 * since it grows, and its `link_in` (`link/wit/types.wit`), which the
 * runtime's own wrapping makes trap the link where the guest traps. */
export interface Guest {
  readonly mem: ArrayBuffer;
  in(): i32;
}

/** The host's end of the link over the guest's boxes, as xpute-link
 * `link.rs` `Side` is either side: the end (`End`), and its transport, the
 * frames it lays in the guest's inbox, crossing by `link_in`, and takes from
 * the guest's outbox at its import (`recv`), by the layout the guest laid
 * (`set_layout`); and the guest's memory, where what it lends is read in
 * place. */
export class Side extends End implements Memory {
  private readonly _guest: Guest;
  private _inbox = 0;
  private _cap = 0;

  constructor(guest: Guest) {
    super();
    this._guest = guest;
  }

  /** Reads the link's layout the guest laid at `at`, an address, a u64
   * (frame.json5 `LAYOUT`); a module's addresses are offsets into its
   * memory, which a number holds. */
  set_layout(at: u64): void {
    const w = new Uint32Array(this._guest.mem, Number(at), LAYOUT.WORDS);
    this._inbox = Number(join64(w[LAYOUT.INBOX_AT]!, w[LAYOUT.INBOX_AT + 1]!));
    this._cap = w[LAYOUT.CAP_AT]!;
  }

  /** The guest's memory, as it stands now: it grows, so it is read anew. */
  get mem(): ArrayBuffer {
    return this._guest.mem;
  }

  /** The `len` bytes the guest lent at `at`, its address (link/wit/types.wit
   * `lent`): a module's offset into its memory. */
  bytes(at: u64, len: u32): U8Array {
    return this._bytes(Number(at), len);
  }

  /** The frame the guest laid at `at`, at its import: a later answer, heard
   * with no data where its packet is past the cap, or a method, its answer
   * laid in the frame it came in. A request's packet past the cap is
   * refused, EBADMSG; an answer past it EMSGSIZE (xpute-link `link.rs`
   * `Side::recv`). */
  recv(at: u64): i32 {
    const off = Number(at);
    const f = read(new DataView(this._guest.mem), off);
    const args =
      f.pkt_len === 0
        ? null
        : HDR_SZ + f.pkt_len > this._cap
          ? ERRNO_EBADMSG
          : this._bytes(off + HDR_SZ, f.pkt_len);
    if (f.res) {
      const data = typeof args === "number" ? null : args;
      return this.input(f.tag, f.handle, f.method, true, f.result, data) as i32;
    }
    const reply =
      typeof args === "number"
        ? args
        : this.input(f.tag, f.handle, f.method, false, f.result, args);
    // The args are read: the answer goes in the frame they came in.
    const { result, data } =
      typeof reply === "number" ? { result: reply, data: null } : reply;
    // An answer past the cap is refused as a later one is (`_out`).
    if (this._lay(off, f.tag, f.handle, f.method, true, result, data) !== 0) {
      this._lay(off, f.tag, f.handle, f.method, true, ERRNO_EMSGSIZE, null);
      return ERRNO_EMSGSIZE;
    }
    return result;
  }

  /** The `len` bytes at `off` in the guest's memory. */
  private _bytes(off: number, len: u32): U8Array {
    return new Uint8Array(this._guest.mem, off, len);
  }

  /** Lays a frame at `at` in the guest's memory, with the packet `args`: 0,
   * or EMSGSIZE, laying nothing, where the packet is past the cap
   * (xpute-link link.rs `Side::lay`). */
  private _lay(
    at: number,
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
   * cap is refused: a packet written elsewhere, as one a runtime carried
   * from another thread, is its sender's fault, answered EMSGSIZE; a later
   * answer's is answered EMSGSIZE with no data, so its request still hears;
   * args this side writes past it are a bug (xpute-link `link.rs`
   * `Side::out`). */
  protected _out(
    tag: Tag,
    h: Handle,
    m: Method,
    res: boolean,
    result: i32,
    args: Packet,
  ): Answered {
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
        ? this._bytes(this._inbox + HDR_SZ, f.pkt_len)
        : null;
    return { result: answered, data };
  }
}
