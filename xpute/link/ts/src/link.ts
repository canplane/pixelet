// @xpute/link/link.ts

/**
 * The host's link to a guest, alike the guest's (xpute-link link.rs,
 * link/wit/types.wit). Whatever is called is an object, called by its methods
 * under a handle: an interface's named handle, the hash of its name (frame.json
 * `NAMED`), or a resource's, named by a URI, a tuple of tokens, and opened by
 * the root's `open` to a handle the serving side issues. A frame is one method called: its tag the request,
 * unique among the sender's pending ones, its command the handle, the method
 * and a RES bit (frame.ts), its packet the args in XTP. A frame crosses one way
 * through one function, and each crossing returns at once what the method came
 * to: its result, or EINPROGRESS for one that answers later, by a frame marked
 * RES with the request's tag. `call` takes what the crossing returns, for a
 * method that always answers at once, and stops as a bug on EINPROGRESS; `send`
 * hands it to its answer, and an `async func` is sent; `mount_named` serves an
 * interface under its named handle, and `mount` puts the opener of the
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
 *   marked RES, with its result and its return data as the packet. A call's
 *   return data is readable until the next crossing; a send's answer gets a
 *   copy, which it owns.
 */

import type { U8Array } from "@xpute/kit/abi/array.ts";
import type { i32, u32 } from "@xpute/kit/abi/word.ts";
import { Errno } from "@xpute/kit/status/errno.spec.ts";
import { InvariantError, MarshalError, XputeError } from "@xpute/kit/status/error.ts";
import { type BranchCursor, type NodeCursor, type NodeType, PacketWriter, TreeReader } from "@xpute/kit/wire/xtp/mod.ts";
import { read_Token, type Token, write_Token } from "./abi/types.ts";
import { lay, read, WORD_BYTES } from "./frame.ts";
import { CANCEL, DROP, NAMED, OPEN, ROOT } from "./frame.spec.ts";
import { type Handle, Table } from "./resource.ts";

export type Tag = u32;
export type Method = u32;
export type { Handle } from "./resource.ts";

export { CANCEL, DROP, NAMED, OPEN, ROOT };

/** An object the host serves: given a method, the request's tag, for a
 * later answer (`task_return`), and the args, it answers. */
export interface LinkObject {
  call(m: Method, tag: Tag, args: BranchCursor | null): i32 | Reply;
}

/** What opens the object a URI names, given the handle it is issued. */
export type Opener = (uri: readonly Token[], h: Handle) => LinkObject;

/** A method's args, or its return data: how many fields, and what writes
 * them. Null, or no fields, is no packet (xpute-link link.rs `Args`). */
export type Args = readonly [u32, (w: PacketWriter) => void] | null;

/** What a method answers: its result, a status or a value, and its return
 * data, written once its args are read. */
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

/** The node `c` as `T`, a node of `type`; any other refuses the packet with
 * EBADMSG, as the kit's checked getters do. */
export function get_as<T>(c: NodeCursor, type: NodeType): T {
  if (c.type !== type) throw new MarshalError(Errno.EBADMSG);
  return c.get() as T;
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

/** Opens the object `uri` names on the other side, and answers its handle. */
export async function open(link: Sender, uri: readonly Token[]): Promise<Handle> {
  const a = await link.send(ROOT, OPEN, [1, (w) => w.branch(uri.length, (w) => uri.forEach((t) => write_Token(w, t)))]);
  if (a.result < 0) throw new XputeError(a.result);
  return a.result;
}

/** Lets the object `h` names on the other side go. */
export function drop(link: Sender, h: Handle): void {
  void link.send(h, DROP, null);
}

/** The errno a method refuses with: an XputeError's. Anything else is a bug,
 * thrown on. */
export function refusal_of(e: unknown): i32 {
  if (e instanceof XputeError && !(e instanceof InvariantError)) return e.errnum;
  throw e;
}

/** What the host holds of its guest: the guest's memory, and its three
 * exports (link/wit/types.wit). */
export interface Guest {
  readonly mem: ArrayBuffer;
  /** link_init: where the link's layout lies, the inbox and a frame's room. */
  init(): u32;
  /** link_boot: the guest's first entry, which boots it. */
  boot(): void;
  /** link_in: the frame laid in the inbox. */
  in(): i32;
}

export class Link {
  /** The openers, by the URI's leading tokens. */
  private readonly _mounts: [readonly Token[], Opener][] = [];
  /** The objects the host serves, by the handles it issued. */
  private readonly _objects = new Table<LinkObject>();
  /** The interfaces the host serves, by their named handles (`NAMED`). */
  private readonly _named = new Map<Handle, LinkObject>();
  /** The answers of the sent requests that answer later, by tag. */
  private readonly _pending = new Map<Tag, Answer>();
  /** How each request the host serves stops if its caller cancels it, by
   * tag, until it answers. */
  private readonly _cancels = new Map<Tag, () => void>();
  private _next: Tag = 0;
  private readonly _guest: Guest;
  private _inbox = 0;
  private _room = 0;

  /** The guest's import, which a module takes under `env`. */
  static imports(link: () => Link) {
    return { link_out: (at: u32): i32 => link()._recv(at) };
  }

  constructor(guest: Guest) {
    this._guest = guest;
  }

  /** Enters the guest first, once the host's own objects are mounted: the
   * guest lays its link, the host reads its layout, and then the guest
   * boots, and may open them. */
  init(): void {
    const at = this._guest.init();
    const layout = new DataView(this._guest.mem, at, 8);
    this._inbox = layout.getUint32(0, true);
    this._room = layout.getUint32(4, true);
    this._guest.boot();
  }

  mount(prefix: readonly Token[], open: Opener): void {
    this._mounts.push([prefix, open]);
  }

  call(h: Handle, m: Method, args: Packet = null): Answered {
    const answered = this._out(this._tag(), h, m, false, 0, args);
    if (answered.result === Errno.EINPROGRESS) throw new InvariantError(Errno.EINPROGRESS);
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
    if (result === Errno.EINPROGRESS || !this._pending.delete(t) || answer === undefined) return;
    const copy = data && data.slice();
    queueMicrotask(() => answer(result, copy));
  }

  /** Answers the request `t`, whose method answered EINPROGRESS, with
   * `result` and its return data, as the component model's `task.return`. A
   * later answer is found by its tag alone. */
  task_return(t: Tag, result: i32, data: Args = null): void {
    this._cancels.delete(t);
    this._out(t, ROOT, 0, true, result, data);
  }

  /** Serves `o` under its interface's named handle `h`, which the guest
   * calls with no open. */
  mount_named(h: Handle, o: LinkObject): void {
    if (this._named.has(h)) throw new InvariantError(Errno.EEXIST);
    this._named.set(h, o);
  }

  /** Puts `stop` as what runs if the caller cancels the request `t` the host
   * serves (frame.json `CANCEL`): a method that can stop then answers
   * ECANCELED, and one that can't runs on. */
  on_cancel(t: Tag, stop: () => void): void {
    this._cancels.set(t, stop);
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
      const data = f.pkt_len > 0 ? this.bytes(at + WORD_BYTES, f.pkt_len).slice() : null;
      if (a !== undefined) queueMicrotask(() => a(f.result, data));
      return 0;
    }
    const args = this._args(at, f.pkt_len);
    const reply = typeof args === "number" ? args : this._serve(f.handle, f.method, f.tag, args);
    // The args are read: the answer goes in the frame they came in.
    const { result, data } = typeof reply === "number" ? { result: reply, data: null } : reply;
    // An answer past the room is refused as a later one is (`_out`).
    if (this._lay(at, f.tag, f.handle, f.method, true, result, data) !== 0) {
      this._lay(at, f.tag, f.handle, f.method, true, Errno.EMSGSIZE, null);
      return Errno.EMSGSIZE;
    }
    return result;
  }

  /** Runs `m` of `h`: the root's open or cancel, a drop, or an object's
   * method. */
  private _serve(h: Handle, m: Method, tag: Tag, args: BranchCursor | null): i32 | Reply {
    if (h === ROOT && m === OPEN) {
      try {
        return this._opened(args);
      } catch (e) {
        return refusal_of(e);
      }
    }
    if (h === ROOT && m === CANCEL) {
      try {
        if (args === null) return Errno.EBADMSG;
        const t = args.at(0).get_number();
        const stop = this._cancels.get(t);
        this._cancels.delete(t);
        stop?.();
        return 0;
      } catch (e) {
        return refusal_of(e);
      }
    }
    if (m === DROP) return this._objects.remove(h) === undefined ? Errno.EBADF : 0;
    // A named handle no interface is mounted under is no object here; an
    // issued one no longer held is a bad handle.
    if ((h & NAMED) !== 0) {
      const o = this._named.get(h);
      return o === undefined ? Errno.ENOENT : o.call(m, tag, args);
    }
    const o = this._objects.get(h);
    return o === undefined ? Errno.EBADF : o.call(m, tag, args);
  }

  /** Opens the object the args' URI names, by the opener of its longest
   * matching mount, and issues its handle. */
  private _opened(args: BranchCursor | null): Handle {
    if (args === null) throw new MarshalError(Errno.EBADMSG);
    const uri = args.at_branch(0).children().map(read_Token);
    let best: [readonly Token[], Opener] | undefined;
    for (const m of this._mounts) {
      const [p] = m;
      const leads = p.length <= uri.length && p.every((t, k) => t.tag === uri[k]!.tag && t.val === uri[k]!.val);
      if (leads && (best === undefined || p.length > best[0].length)) best = m;
    }
    if (best === undefined) throw new XputeError(Errno.ENOENT);
    const h = this._objects.issue();
    try {
      this._objects.put(h, best[1](uri, h));
    } catch (e) {
      this._objects.remove(h);
      throw e;
    }
    return h;
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

  /** The args of the frame at `at`, or the errno they fail to decode with: a
   * packet past the room is EBADMSG. */
  private _args(at: u32, pkt_len: u32): BranchCursor | null | i32 {
    if (pkt_len === 0) return null;
    if (WORD_BYTES + pkt_len > this._room) return Errno.EBADMSG;
    try {
      return new TreeReader(this.bytes(at + WORD_BYTES, pkt_len)).read_branch();
    } catch (e) {
      if (!(e instanceof MarshalError)) throw e;
      return e.errnum;
    }
  }

  /** Lays a frame at `at` in the guest's memory, with the packet `args`: 0,
   * or EMSGSIZE, laying nothing, where the packet is past the room
   * (xpute-link link.rs `Side::lay`). */
  private _lay(at: u32, tag: Tag, handle: Handle, method: Method, res: boolean, result: i32, args: Packet): i32 {
    const bytes = new Uint8Array(this._guest.mem, at, this._room);
    const view = new DataView(this._guest.mem, at, this._room);
    let pkt_len = 0;
    if (args instanceof Uint8Array) {
      if (WORD_BYTES + args.length > this._room) return Errno.EMSGSIZE;
      bytes.set(args, WORD_BYTES);
      pkt_len = args.length;
    } else if (args !== null && args[0] > 0) {
      const w = new PacketWriter().begin(bytes, view, WORD_BYTES, this._room - WORD_BYTES, args[0]);
      args[1](w);
      pkt_len = w.finish();
      if (pkt_len < 0) return Errno.EMSGSIZE;
    }
    lay(view, 0, { tag, res, handle, method, result, pkt_len });
    return 0;
  }

  /** Lays a frame in the guest's inbox and crosses with it, and answers what
   * the guest laid there: its result, and its return data. A packet past the
   * room is refused: a packet written elsewhere, as one from across a port,
   * is its sender's fault, answered EMSGSIZE; a later answer's is answered
   * EMSGSIZE with no data, so its request still hears; args this side
   * writes past it are a bug (xpute-link link.rs `Side::out`). */
  private _out(tag: Tag, h: Handle, m: Method, res: boolean, result: i32, args: Packet): Answered {
    if (this._lay(this._inbox, tag, h, m, res, result, args) !== 0) {
      if (!res) {
        if (!(args instanceof Uint8Array)) throw new InvariantError(Errno.EMSGSIZE);
        return { result: Errno.EMSGSIZE, data: null };
      }
      this._lay(this._inbox, tag, h, m, res, Errno.EMSGSIZE, null);
    }
    const answered = this._guest.in();
    const f = read(new DataView(this._guest.mem), this._inbox);
    const data = f.res && !res && f.pkt_len > 0 ? this.bytes(this._inbox + WORD_BYTES, f.pkt_len) : null;
    return { result: answered, data };
  }
}
