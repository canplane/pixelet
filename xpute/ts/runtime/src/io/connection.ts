// @xpute/runtime/io/connection.ts

/**
 * This is the runtime's end of a core on a page, its connection to the core.
 * One run of the connection is one turn (sched/scheduler.ts). What the runtime sends the core waits here
 * until the core receives it in its next turn, and what the core sends during
 * a turn is kept and read once the turn returns, so a listener never runs
 * inside a turn and never runs the core. The core reaches this through its
 * host (`host`, xpute-core host.rs). A frame's tag is the sender's id
 * (io/frame.json): each call the runtime makes takes the next id, and its
 * reply keeps that id. Everything the core raises is handed to the one
 * listener, which tells the commands apart.
 */

import type { f64, i32, u32 } from "@xpute/kit/abi/word.ts";
import type { U8Array } from "@xpute/kit/abi/array.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { strerror } from "@xpute/kit/status/errno.spec.ts";
import {
  type Arg,
  type BranchCursor,
  type PacketRoom,
  PacketWriter,
  TreeReader,
} from "@xpute/kit/wire/xtp/mod.ts";
import { NO_WAKE } from "../sched/wake.ts";
import { say } from "../say.ts";
import * as frame from "./frame.ts";
import { PKT } from "./frame.spec.ts";
import { InvariantError, XputeError } from "@xpute/kit/status/error.ts";

/** Hears what the core sends. `f` is the packet as read, `pkt` its bytes and
 * `named` the memory the frame names; all are valid only inside the call. A
 * call of the core's is answered with what this returns, or 0 if it returns
 * nothing. A frame the core sends with ACKREQ is completed with what this
 * returns; if it returns nothing, its device completes the frame later
 * (`Connection.complete`). */
export type OnImport = (
  tag: u32,
  cmd: u32,
  f: BranchCursor | null,
  pkt: U8Array | null,
  named: U8Array | null,
) => i32 | void;

/** An `Encode` writes a command's packet into the room it is given. */
export type Encode = (room: PacketRoom) => PacketWriter | null;

type Run = (deadline_ms: f64) => f64;

/** What hears each turn the connection runs (sched/scheduler.ts): the turn, then
 * when the core wants the next. A wake of 0 is heard as well after a read
 * that left frames waiting for the core. */
export interface Heard {
  on_turn(): void;
  on_wake(wake_ms: f64): void;
}

/** This is how a core reaches its runtime (xpute-core host.rs), each function
 * naming a frame in the core's memory by its offset. */
export interface Host {
  receive(at: u32, cap: u32): i32;
  send(at: u32, named_at: u32, named_len: u32): void;
  call(at: u32): i32;
}

/** A request's reply as program code takes it: its value, or the errno the
 * request was refused with, thrown, so no errno reaches it as a number. */
export function answer_of(res: i32): u32 {
  if (res < 0) throw new XputeError(res);
  return res;
}

/** An error as a line says it: what it is, then its stack, which says where
 * the core trapped, as a native runtime's stop report says the panic and its
 * place. One engine's stack begins with what the error is and another's
 * does not, so it is said once either way. */
const told = (err: unknown): string => {
  const what = String(err);
  const stack = err instanceof Error ? (err.stack ?? "") : "";
  return stack.startsWith(what) ? stack : `${what}\n${stack}`.trimEnd();
};

/** The errno a program's handler refused a command with, thrown as kit's
 * error (`answer_of`'s other way); an error that is no refusal, a broken
 * invariant among them, goes on. */
export function refusal_of(err: unknown): i32 {
  if (err instanceof XputeError && !(err instanceof InvariantError)) {
    return err.errnum;
  }
  throw err;
}

/** The host before a runtime sets one: a core that reaches it breaks the
 * boundary, and stops with ENOSYS. */
export const UNSET: Host = {
  receive: unset,
  send: unset,
  call: unset,
};

function unset(): never {
  throw new InvariantError(Errno.ENOSYS);
}

/** A frame the core sent, copied out of its memory, with the memory it
 * named. */
interface Sent extends frame.Frame {
  named: U8Array | null;
}

/** Writes a packet whose fields are `args`, in order, in place. With no
 * arguments there is no packet. */
export function encode_args(args: Arg[]): Encode {
  return (room) => {
    if (args.length === 0) return null;
    const pkt = room.writer(args.length);
    for (const a of args) pkt.arg(a);
    return pkt;
  };
}

export class Connection implements PacketRoom {
  private readonly _mem: ArrayBuffer;
  /** Every packet is written with this one writer, into `_room`, and copied
   * out once written. */
  private readonly _writer = new PacketWriter();
  private readonly _room: U8Array;
  private readonly _room_view: DataView;
  /** What waits for the core, oldest first. */
  private readonly _waiting: frame.Frame[] = [];
  /** What the core sent this turn, oldest first. */
  private _sent: Sent[] = [];
  /** The tag of the UI's last request; 0 is a send's. */
  private _next_tag: u32 = 0;
  private readonly _replies = new Map<u32, (result: i32) => void>();
  /** This is set while what the core sent is read, and while a listener
   * answers the core's call, so that nothing runs the core then. */
  private _reading = false;
  /** The core is gone. Either its turn stopped partway, or the runtime could
   * not take what it sent; either way the submission it was applying would
   * end it again on every run, so it is never run again. */
  private _terminated = false;
  /** Every call fails with ENOSYS until the runtime listens. */
  private _listener: OnImport = () => Errno.ENOSYS;
  /** This is told once, as the core stops. */
  onterminated: (() => void) | null = null;

  private readonly _turn: Run;
  /** It hears each turn before what the turn sent is read, so that whatever
   * that read asks for outlives the turn. */
  private readonly _heard: Heard;

  /** The core's host, which the core's platform hands the core. */
  readonly host: Host;

  get terminated(): boolean {
    return this._terminated;
  }

  /** Whether frames wait for the core's next turn. */
  get waiting(): boolean {
    return this._waiting.length > 0;
  }

  /** `pkt_bytes` is the most a frame's packet takes, either way. */
  constructor(mem: ArrayBuffer, pkt_bytes: u32, run: Run, heard: Heard) {
    this._mem = mem;
    this._turn = run;
    this._heard = heard;
    this._room = new Uint8Array(new ArrayBuffer(pkt_bytes));
    this._room_view = new DataView(this._room.buffer);
    this.host = {
      receive: (at, cap) => this._receive(at, cap),
      send: (at, named_at, named_len) => this._send(at, named_at, named_len),
      call: (at) => this._answer(at),
    };
  }

  /** The writer, begun over the room for a packet of `cnt` children. It is
   * valid until the next packet is written. */
  writer(cnt: u32): PacketWriter {
    this._writer.begin(
      this._room,
      this._room_view,
      0,
      this._room.byteLength,
      cnt,
    );
    return this._writer;
  }

  /** Whether a command carrying `pkt` can be sent at all. A command that
   * cannot is input the caller must refuse, at the boundary it came
   * through. */
  fits(pkt: PacketWriter): boolean {
    return pkt.finish() >= 0;
  }

  listen(listener: OnImport): void {
    this._listener = listener;
  }

  /** Sends `cmd` tagged 0, for the core's next turn. */
  send(cmd: u32, encode: Encode | null = null): void {
    const errnum = this._submit(0, cmd, encode, 0);
    if (errnum !== Errno.OK) throw new InvariantError(errnum);
  }

  /** Sends `cmd` with ACKREQ, tagged with the next call's id. The promise
   * settles with the reply the core sends as it applies the request, which
   * is read as its turn returns; nothing blocks here. A core that has
   * stopped answers ENOTRECOVERABLE, and every request still waiting is
   * answered so when the core stops. A packet larger than a frame's room
   * answers EMSGSIZE. */
  request(cmd: u32, encode: Encode | null = null): Promise<i32> {
    if (this._terminated) return Promise.resolve(Errno.ENOTRECOVERABLE);
    this._next_tag = (this._next_tag + 1) >>> 0 || 1;
    const tag = this._next_tag;
    return new Promise((resolve) => {
      this._replies.set(tag, (result) => resolve(result));
      const errnum = this._submit(tag, cmd, encode, frame.ACKREQ);
      if (errnum !== Errno.OK) {
        this._replies.delete(tag);
        resolve(errnum);
      }
    });
  }

  /** Completes the core's request tagged `tag`, which it sent with ACKREQ,
   * by sending a frame of that tag and command, flagged RES, with `result`
   * and the packet `encode` writes. */
  complete(
    tag: u32,
    cmd: u32,
    result: i32,
    encode: Encode | null = null,
  ): void {
    const errnum = this._submit(tag, cmd, encode, frame.RES, result);
    if (errnum !== Errno.OK) throw new InvariantError(errnum);
  }

  /** Sends `cmd` with `args` as its packet's fields, in order, and runs at
   * once. It is for a command from outside the runtime's own code, such as a
   * console's. */
  cmd(cmd: u32, ...args: Arg[]): void {
    this.send(cmd, encode_args(args));
    this.run(0);
  }

  run(deadline_ms: f64): f64 {
    if (this._reading) throw new InvariantError(Errno.EBUSY);
    if (this._terminated) return NO_WAKE;
    let wake: f64;
    try {
      wake = this._turn(deadline_ms);
    } catch (err) {
      // What the core sent before it trapped, its stop report among it, is
      // still read.
      this._terminate("the core trapped", err, true);
      return NO_WAKE;
    }
    this._heard.on_turn();
    try {
      this._read();
    } catch (err) {
      // The runtime could not take a frame: the core broke the protocol, or
      // what answers it failed. Nothing after that frame can be read.
      this._terminate(
        "the runtime could not take what the core sent",
        err,
        false,
      );
      return NO_WAKE;
    }
    this._heard.on_wake(wake);
    return wake;
  }

  /** Ends the core. The cause is said here, once (say.ts), and the stop is
   * told, because whoever ran the turn is a timer or an import's handler that
   * can do nothing about it. With `read`, what the core sent is read first. */
  private _terminate(why: string, err: unknown, read: boolean): void {
    say(`[xpute/connection] ${why}: ${told(err)}`);
    this._terminated = true;
    try {
      if (read) this._read();
      const replies = [...this._replies.values()];
      this._replies.clear();
      for (const r of replies) r(Errno.ENOTRECOVERABLE);
    } catch (err) {
      say(
        `[xpute/connection] the runtime could not take what the core sent: ${told(err)}`,
      );
    } finally {
      this.onterminated?.();
    }
  }

  /** The core receives the next frame that waits for it, laid at `at` in the
   * `cap` bytes there. Room that is not the core's (`frame.in_core`), or at
   * offset 0, is the core's bug, and stops it with EFAULT. */
  private _receive(at: u32, cap: u32): i32 {
    if (at === 0 || !frame.in_core(this._mem, at, cap)) {
      throw new InvariantError(Errno.EFAULT);
    }
    const f = this._waiting.shift();
    if (f === undefined) return 0;
    if (frame.WORD_BYTES + (f.pkt?.byteLength ?? 0) > cap) {
      throw new InvariantError(Errno.EMSGSIZE);
    }
    frame.lay_frame(this._mem, at, f);
    return 1;
  }

  /** Takes the frame the core laid at `at` and the memory it names. A frame
   * or a packet that is not the core's, the frame at offset 0 among them, and
   * named memory that is not the core's (`frame.in_core`), are the core's
   * bug, and stop it with EFAULT. */
  private _send(at: u32, named_at: u32, named_len: u32): void {
    const mem = this._mem;
    if (!frame.in_core(mem, at, frame.WORD_BYTES))
      throw new InvariantError(Errno.EFAULT);
    const pkt_len = new DataView(mem).getUint32(at + 4 * PKT, true);
    if (!frame.in_core(mem, at, frame.WORD_BYTES + pkt_len)) {
      throw new InvariantError(Errno.EFAULT);
    }
    if (!frame.in_core(mem, named_at, named_len)) {
      throw new InvariantError(Errno.EFAULT);
    }
    this._sent.push({
      ...copied(frame.frame_at(mem, at)),
      named:
        named_len === 0
          ? null
          : new Uint8Array(mem, named_at, named_len).slice(),
    });
  }

  /** Answers the core's call, the frame laid at `at`. The listener answers
   * it before the core goes on. This runs inside the core's turn, so the
   * answer must run no turn. */
  private _answer(at: u32): i32 {
    // A frame that is not the core's, or at offset 0, is the core's bug.
    if (!frame.in_core(this._mem, at, frame.WORD_BYTES)) {
      throw new InvariantError(Errno.EFAULT);
    }
    const f = frame.frame_at(this._mem, at);
    const cursor = f.pkt ? new TreeReader(f.pkt).read_branch() : null;
    this._reading = true;
    try {
      return this._listener(f.tag, f.cmd, cursor, f.pkt, null) ?? 0;
    } finally {
      this._reading = false;
    }
  }

  private _read(): void {
    const sent = this._sent;
    this._sent = [];
    this._reading = true;
    try {
      for (const s of sent) {
        const f = s.pkt ? new TreeReader(s.pkt).read_branch() : null;
        if (s.flags & frame.RES) {
          const reply = this._replies.get(s.tag);
          if (reply !== undefined) {
            this._replies.delete(s.tag);
            reply(s.result);
          } else if (s.result < 0) {
            console.warn(
              `[xpute/connection] command 0x${s.cmd.toString(16)} failed: ${strerror(
                s.result,
              )}`,
            );
          }
        } else {
          const result = this._listener(s.tag, s.cmd, f, s.pkt, s.named);
          if (result !== undefined && (s.flags & frame.ACKREQ || result < 0)) {
            this.complete(s.tag, s.cmd, result);
          }
        }
      }
    } finally {
      this._reading = false;
    }
    // What a listener sent or completed waits for the core, so a turn is
    // asked for it.
    if (this._waiting.length > 0) this._heard.on_wake(0);
  }

  /** Writes the packet `encode` writes and queues the frame for the core.
   * Returns EMSGSIZE for a packet larger than a frame's room. */
  private _submit(
    tag: u32,
    cmd: u32,
    encode: Encode | null,
    flags: u32,
    result: i32 = 0,
  ): i32 {
    if (this._terminated) return Errno.OK;
    const w = encode?.(this) ?? null;
    let pkt: U8Array | null = null;
    if (w) {
      const end = w.finish();
      if (end < 0) return Errno.EMSGSIZE;
      pkt = this._room.slice(0, end);
    }
    this._waiting.push({ tag, cmd, flags, result, pkt });
    return Errno.OK;
  }
}

/** The frame with its packet copied out of the memory it lies in. */
function copied(f: frame.Frame): frame.Frame {
  return { ...f, pkt: f.pkt && f.pkt.slice() };
}
