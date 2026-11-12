// @xpute/runtime/link.ts

/**
 * The runtime's link to a core, alike the core's (xpute-core link.rs,
 * wit/link.wit). Whatever is called is an object: named by a URI, a tuple of
 * tokens, opened by the root's `open` to a handle the serving side issues,
 * and called by its methods. A frame is one method called: its tag the
 * request, unique among the sender's pending ones, its command the handle,
 * the method and a RES bit (io/frame.ts), its packet the args in XTP. A frame
 * crosses one way through one function, and each crossing returns at once
 * what the method came to: its result, or EINPROGRESS for one that answers
 * later, by a frame marked RES with the request's tag. `call` takes what the
 * crossing returns, for a method that always answers at once, and stops as a
 * bug on EINPROGRESS; `send` hands it to its answer, and an `async func` is
 * sent; `mount` puts the opener of the objects the runtime serves under a
 * URI's leading tokens, the longest matching winning.
 *
 * The link holds these on both ends:
 *
 * - One core entry at a time: no `link_in` starts before another returns,
 *   and the outside never enters the core nested.
 * - An answer is never run inside `send`, and a result at once and a later
 *   one take the same path. On the core, the answers run at the end of the
 *   entry, once its handler has returned, just before `link_in` returns; on
 *   the runtime, after the work at hand (`queueMicrotask`).
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
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { InvariantError, MarshalError, XputeError } from "@xpute/kit/status/error.ts";
import { type Arg, Branch, type BranchCursor, PacketWriter, TreeReader } from "@xpute/kit/wire/xtp/mod.ts";
import { read_Token, type Token, write_Token } from "./abi/link.ts";
import { lay, read, WORD_BYTES } from "./io/frame.ts";
import { type Handle, Table } from "./resource.ts";

export type Tag = u32;
export type Method = u32;
export type { Handle } from "./resource.ts";

/** The root's handle, and its one method. */
export const ROOT: Handle = 0;
export const OPEN: Method = 0;
/** The method every handle has, which lets it go. */
export const RELEASE: Method = 0xff;

/** An object the runtime serves: given a method, the request's tag, for a
 * later answer (`complete`), and the args, it answers. */
export interface LinkObject {
  call(m: Method, tag: Tag, args: BranchCursor | null): i32 | Reply;
}

/** What opens the object a URI names. */
export type Opener = (uri: readonly Token[]) => LinkObject;

/** What a method answers: its result, a status or a value, and its return
 * data, written once its args are read. */
export interface Reply {
  result: i32;
  data: readonly Arg[];
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
export type Packet = readonly Arg[] | U8Array;

/** What sends a method to the other side and is answered by promise: a UI's
 * `Remote`, or a link through `wait`. */
export interface Sender {
  send(h: Handle, m: Method, args: readonly Arg[]): Promise<Answered>;
}

/** A reference to an object on the other side: the link it is reached
 * over, and the handle that side issued. */
export interface Ref {
  readonly link: Sender;
  readonly handle: Handle;
}

/** The URI of names alone, as a singleton's (package, interface). */
export function uri(...names: string[]): Token[] {
  return names.map((val) => ({ tag: "str", val }));
}

/** Opens the object `uri` names on the other side, and answers its handle. */
export async function open(link: Sender, uri: readonly Token[]): Promise<Handle> {
  const a = await link.send(ROOT, OPEN, [new Branch(uri.map(write_Token))]);
  if (a.result < 0) throw new XputeError(a.result);
  return a.result;
}

/** Lets the object `h` names on the other side go. */
export function release(link: Sender, h: Handle): void {
  void link.send(h, RELEASE, []);
}

/** The errno a method refuses with: an XputeError's. Anything else is a bug,
 * thrown on. */
export function refusal_of(e: unknown): i32 {
  if (e instanceof XputeError && !(e instanceof InvariantError)) return e.errnum;
  throw e;
}

/** What a core gives its runtime: its memory, and its two exports. */
export interface Core {
  readonly mem: ArrayBuffer;
  /** link_init: where the link's layout lies, the inbox and a frame's room. */
  init(): u32;
  /** link_in: the frame laid in the inbox. */
  in(): i32;
}

export class Link {
  /** The openers, by the URI's leading tokens. */
  private readonly _mounts: [readonly Token[], Opener][] = [];
  /** The objects the runtime serves, by the handles it issued. */
  private readonly _objects = new Table<LinkObject>();
  /** The answers of the sent requests that answer later, by tag. */
  private readonly _pending = new Map<Tag, Answer>();
  private _next: Tag = 0;
  private readonly _core: Core;
  private _inbox = 0;
  private _room = 0;

  /** The core's import, which a module takes under `env`. */
  static imports(link: () => Link) {
    return { link_out: (at: u32): i32 => link()._recv(at) };
  }

  constructor(core: Core) {
    this._core = core;
  }

  /** Enters the core first, once the runtime's own objects are mounted: the
   * core lays its link and boots, and may open them. */
  init(): void {
    const at = this._core.init();
    const layout = new DataView(this._core.mem, at, 8);
    this._inbox = layout.getUint32(0, true);
    this._room = layout.getUint32(4, true);
  }

  mount(prefix: readonly Token[], open: Opener): void {
    this._mounts.push([prefix, open]);
  }

  call(h: Handle, m: Method, args: Packet = []): Answered {
    const answered = this._out(this._tag(), h, m, false, 0, args);
    if (answered.result === Errno.EINPROGRESS) throw new InvariantError(Errno.EINPROGRESS);
    return answered;
  }

  /** Sends `m` of `h`, its answer pending from before the crossing, so a
   * later answer the callee gives inside it finds its request. */
  send(h: Handle, m: Method, args: Packet = [], answer?: Answer): void {
    const t = this._tag();
    if (answer !== undefined) this._pending.set(t, answer);
    const { result, data } = this._out(t, h, m, false, 0, args);
    if (result === Errno.EINPROGRESS || !this._pending.delete(t) || answer === undefined) return;
    const copy = data && data.slice();
    queueMicrotask(() => answer(result, copy));
  }

  /** Answers the request `t`, whose method answered EINPROGRESS, with
   * `result` and its return data. A later answer is found by its tag alone. */
  complete(t: Tag, result: i32, data: readonly Arg[] = []): void {
    this._out(t, ROOT, 0, true, result, data);
  }

  /** The `len` bytes at `at` in the core's memory. */
  bytes(at: u32, len: u32): U8Array {
    return new Uint8Array(this._core.mem, at, len);
  }

  /** The frame the core laid at `at`: a later answer, to its request, or a
   * method, to its object. */
  private _recv(at: u32): i32 {
    const f = read(new DataView(this._core.mem), at);
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
    const { result, data } = typeof reply === "number" ? { result: reply, data: [] } : reply;
    this._lay(at, f.tag, f.handle, f.method, true, result, data);
    return result;
  }

  /** Runs `m` of `h`: the root's open, a release, or an object's method. */
  private _serve(h: Handle, m: Method, tag: Tag, args: BranchCursor | null): i32 | Reply {
    if (h === ROOT && m === OPEN) {
      try {
        return this._opened(args);
      } catch (e) {
        return refusal_of(e);
      }
    }
    if (m === RELEASE) return this._objects.remove(h) === undefined ? Errno.EBADF : 0;
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
    return this._objects.add(best[1](uri));
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

  /** Lays a frame at `at` in the core's memory, with the packet `args`. */
  private _lay(at: u32, tag: Tag, handle: Handle, method: Method, res: boolean, result: i32, args: Packet): void {
    const bytes = new Uint8Array(this._core.mem, at, this._room);
    const view = new DataView(this._core.mem, at, this._room);
    let pkt_len = 0;
    if (args instanceof Uint8Array) {
      if (WORD_BYTES + args.length > this._room) throw new InvariantError(Errno.EMSGSIZE);
      bytes.set(args, WORD_BYTES);
      pkt_len = args.length;
    } else if (args.length > 0) {
      const w = new PacketWriter().begin(bytes, view, WORD_BYTES, this._room - WORD_BYTES, args.length);
      for (const a of args) w.arg(a);
      pkt_len = w.finish();
      if (pkt_len < 0) throw new InvariantError(Errno.EMSGSIZE);
    }
    lay(view, 0, { tag, res, handle, method, result, pkt_len });
  }

  /** Lays a frame in the core's inbox and crosses with it, and answers what
   * the core laid there: its result, and its return data. */
  private _out(tag: Tag, h: Handle, m: Method, res: boolean, result: i32, args: Packet): Answered {
    this._lay(this._inbox, tag, h, m, res, result, args);
    const answered = this._core.in();
    const f = read(new DataView(this._core.mem), this._inbox);
    const data = f.res && !res && f.pkt_len > 0 ? this.bytes(this._inbox + WORD_BYTES, f.pkt_len) : null;
    return { result: answered, data };
  }
}
