// @xpute/runtime/ipc/doorbell.ts

/**
 * This is the runtime's end of a core's rings. One ring of the doorbell is one
 * turn (sched/scheduler.ts). Everything a turn leaves is read before the next
 * turn is rung, so a listener must not ring the core while the completion ring
 * is being read. A frame's tag is the sender's id (ipc/frame.json): each call
 * the runtime makes takes the next id, and its reply keeps that id. Everything
 * the core raises is handed to the one listener, which tells the commands
 * apart.
 *
 * A core on a thread of its own (runtime.ts `Core.thread`) runs by itself. It
 * is kicked after each submission, and after each read of the completion
 * ring, since a read may have made the room a turn waits for. What it posts is
 * read when it says it has posted (`turns`). For such a core, a ring is a turn
 * waited for, as a call needs. A core that runs only when rung is instead
 * asked for a turn after any read that took frames, so that it sees the room
 * that moving the head gave back.
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
import * as frame from "./frame.ts";
import { Ring } from "./ring.ts";
import { InvariantError } from "@xpute/kit/status/error.ts";

/** Packets for a stopped core are written into this empty buffer, since its
 * sends are dropped. */
const NOWHERE = new Uint8Array(0);
const NOWHERE_VIEW = new DataView(NOWHERE.buffer);

/** Hears what the core sends. `f` is the packet as read and `pkt` its bytes;
 * both are valid only inside the call. A call of the core's is answered with
 * what this returns, or 0 if it returns nothing. A frame the core posts with
 * ACKREQ is completed with what this returns; if it returns nothing, its
 * device completes the frame later (`Doorbell.complete`). */
export type OnImport = (
  tag: u32,
  cmd: u32,
  f: BranchCursor | null,
  pkt: U8Array | null,
) => i32 | void;

/** An `Encode` writes a command's packet into the room it is given. */
export type Encode = (room: PacketRoom) => PacketWriter | null;

type Run = (deadline_ms: f64) => f64;
type Kick = (deadline_ms: f64) => void;

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

export class Doorbell implements PacketRoom {
  private readonly _sq: Ring;
  /** Every packet is written with this one writer, in place in its slot. */
  private readonly _writer = new PacketWriter();
  private readonly _cq: Ring;
  private _next_call: u32 = 0;
  private readonly _waiting = new Map<
    u32,
    (result: i32, f: BranchCursor | null) => void
  >();
  /** This is set while the completion ring is being read, and so while a
   * listener runs. */
  private _reading = false;
  /** The core is gone. Either its turn stopped partway, or the runtime could
   * not take what it posted; either way the submission it was applying would
   * end it again on every ring, so it is never rung again. */
  private _terminated = false;
  /** Every call fails with ENOSYS until the runtime listens. */
  private _listener: OnImport = () => Errno.ENOSYS;
  /** What was sent or completed while the completion ring was being read, as
   * tag, command, flags, result and packet. It is submitted once the read is
   * done, because a packet is written in its slot, and making room for one
   * takes a turn. */
  private _later: [u32, u32, u32, i32, Encode | null][] = [];
  on_wake: ((wake_ms: f64) => void) | null = null;
  /** Told of every turn heard: how long the longest turn took and when the
   * core wants the next. It is told before what the turn posted is read and
   * before `on_wake` hears the same wake, so that whatever that read asks for
   * outlives the turn. A turn heard is each turn rung (`ring`), or whatever
   * the core's own thread ran since it was last heard, whoever kicked it
   * (`turns`). */
  on_turn: ((turn_ms: f64, wake_ms: f64) => void) | null = null;
  /** This is told once, as the core stops. */
  onterminated: (() => void) | null = null;

  private readonly _run: Run;
  /** Wakes a core on its own thread; null for a core that runs only when
   * rung. */
  readonly kick: Kick | null;

  get terminated(): boolean {
    return this._terminated;
  }

  constructor(
    mem: ArrayBufferLike,
    sq_at: u32,
    cq_at: u32,
    run: Run,
    kick: Kick | null = null,
  ) {
    this._run = run;
    this.kick = kick;
    this._sq = new Ring(mem, sq_at);
    this._cq = new Ring(mem, cq_at);
  }

  /** The writer, begun over the next submission's slot for a packet of `cnt`
   * children, after ringing until there is room. It is valid until that
   * submission. */
  writer(cnt: u32): PacketWriter {
    if (this._reading) throw new InvariantError(Errno.EBUSY);
    while (!this._terminated && !this._sq.write(this._writer, cnt))
      this.ring(0);
    if (this._terminated) this._writer.begin(NOWHERE, NOWHERE_VIEW, 0, 0, cnt);
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

  /** Sends `cmd` tagged 0. While the completion ring is being read, the send
   * waits until the read is done. */
  send(cmd: u32, encode: Encode | null = null): void {
    if (this._reading) {
      this._later.push([0, cmd, 0, 0, encode]);
      return;
    }
    const errnum = this._submit(0, cmd, encode, 0);
    if (errnum !== Errno.OK) throw new InvariantError(errnum);
  }

  /** Posts `cmd` with ACKREQ, tagged with the next call's id. The promise
   * settles with the reply the core posts as it applies the request, which
   * is read with the completion ring; nothing blocks here. A core that has
   * stopped answers ENOTRECOVERABLE, and every request still waiting is
   * answered so when the core stops. A packet larger than a slot answers
   * EMSGSIZE. While the completion ring is being read, the request is posted
   * once the read is done. */
  request(cmd: u32, encode: Encode | null = null): Promise<i32> {
    if (this._terminated) return Promise.resolve(Errno.ENOTRECOVERABLE);
    this._next_call = (this._next_call + 1) >>> 0 || 1;
    const tag = this._next_call;
    return new Promise((resolve) => {
      this._waiting.set(tag, (result) => resolve(result));
      if (this._reading) {
        this._later.push([tag, cmd, frame.ACKREQ, 0, encode]);
        return;
      }
      const errnum = this._submit(tag, cmd, encode, frame.ACKREQ);
      if (errnum !== Errno.OK) {
        this._waiting.delete(tag);
        resolve(errnum);
      }
    });
  }

  /** Completes the core's request tagged `tag`, which it posted with ACKREQ,
   * by submitting a frame of that tag and command, flagged RES, with
   * `result` and the packet `encode` writes. While the completion ring is
   * being read, the completion waits until the read is done. */
  complete(
    tag: u32,
    cmd: u32,
    result: i32,
    encode: Encode | null = null,
  ): void {
    if (this._reading) {
      this._later.push([tag, cmd, frame.RES, result, encode]);
      return;
    }
    const errnum = this._submit(tag, cmd, encode, frame.RES, result);
    if (errnum !== Errno.OK) throw new InvariantError(errnum);
  }

  /** Sends `cmd` with `args` as its packet's fields, in order, and rings at
   * once. It is for a command from outside the runtime's own code, such as a
   * console's. */
  cmd(cmd: u32, ...args: Arg[]): void {
    this.send(cmd, encode_args(args));
    this.ring(0);
  }

  ring(deadline_ms: f64): f64 {
    if (this._reading) throw new InvariantError(Errno.EBUSY);
    if (this._terminated) return NO_WAKE;
    let wake: f64;
    const t0 = performance.now();
    try {
      wake = this._run(deadline_ms);
    } catch (err) {
      // What the core posted before it trapped, its stop report among it, is
      // still read.
      this._terminate("the core trapped", err, true);
      return NO_WAKE;
    }
    this.on_turn?.(performance.now() - t0, wake);
    try {
      this._read();
    } catch (err) {
      // The runtime could not take a frame: the core broke the protocol, or
      // what answers it failed. Nothing after that frame can be read.
      this._terminate(
        "the runtime could not take what the core posted",
        err,
        false,
      );
      return NO_WAKE;
    }
    this.on_wake?.(wake);
    return wake;
  }

  /** Called when the core's own thread has posted or asked for a turn since
   * it was last heard. The completion ring is read dry and what the core
   * asked is told. If the core has stopped (`wake_ms` is NaN), what it posted
   * is read and its stop is told. */
  turns(wake_ms: f64, turn_ms: f64): void {
    if (this._terminated) return;
    if (Number.isNaN(wake_ms)) {
      this._terminate("the core stopped", null, true);
      return;
    }
    this.on_turn?.(turn_ms, wake_ms);
    try {
      this._read();
    } catch (err) {
      this._terminate(
        "the runtime could not take what the core posted",
        err,
        false,
      );
      return;
    }
    this.on_wake?.(wake_ms);
  }

  /** Ends the core. The cause is printed here, once, and the stop is told,
   * because whoever rang the turn is a timer or an import's handler that can
   * do nothing about it. With `read`, what the core posted is read first. */
  private _terminate(why: string, err: unknown, read: boolean): void {
    console.error(`[xpute/doorbell] ${why}:`, err);
    this._terminated = true;
    try {
      if (read) this._read();
      const waiting = [...this._waiting.values()];
      this._waiting.clear();
      for (const w of waiting) w(Errno.ENOTRECOVERABLE, null);
    } catch (err) {
      console.error(
        "[xpute/doorbell] the runtime could not take what the core posted:",
        err,
      );
    } finally {
      this.onterminated?.();
    }
  }

  /** Submits `cmd` with ACKREQ, tagged with the next call's id, and rings
   * with a deadline of 0 until the reply returns. The reply is valid only
   * inside `read`. A core that has stopped answers ENOTRECOVERABLE, and a
   * packet larger than a slot answers EMSGSIZE; neither comes with a reply.
   * A core that applied every submission without replying is a protocol
   * error (EPROTO). A call while the completion ring is being read throws
   * EBUSY, because the answer takes a turn. */
  call<T>(
    cmd: u32,
    encode: Encode | null,
    read: (result: i32, reply: BranchCursor | null) => T,
  ): T {
    if (this._reading) throw new InvariantError(Errno.EBUSY);
    if (this._terminated) return read(Errno.ENOTRECOVERABLE, null);
    // Tag 0 belongs to sends, so call ids skip it.
    this._next_call = (this._next_call + 1) >>> 0 || 1;
    const tag = this._next_call;
    let out: { val: T } | null = null;
    this._waiting.set(tag, (result, f) => (out = { val: read(result, f) }));
    try {
      const errnum = this._submit(tag, cmd, encode, frame.ACKREQ);
      if (errnum !== Errno.OK) return read(errnum, null);
      while (out === null) {
        const pending = this._sq.len;
        this.ring(0);
        if (out === null && this._terminated) {
          return read(Errno.ENOTRECOVERABLE, null);
        }
        if (out === null && pending === 0) {
          throw new InvariantError(Errno.EPROTO);
        }
      }
    } finally {
      this._waiting.delete(tag);
    }
    return (out as { val: T }).val;
  }

  /** Answers the core's call: a frame the core laid at `at` in the
   * completion ring without publishing it. The listener answers it before
   * the core goes on. This runs inside the core's turn, so the answer must
   * ring nothing. */
  answer(at: u32): i32 {
    const cq = this._cq;
    const e = cq.entry_of(at);
    const pkt = cq.pkt(e);
    const f: BranchCursor | null = pkt
      ? new TreeReader(pkt).read_branch()
      : null;
    this._reading = true;
    try {
      return this._listener(cq.tag(e), cq.cmd(e), f, pkt) ?? 0;
    } finally {
      this._reading = false;
    }
  }

  private _read(): void {
    this._reading = true;
    let read = 0;
    try {
      const cq = this._cq;
      for (let at = cq.peek(); at >= 0; at = cq.peek()) {
        const tag = cq.tag(at);
        const cmd = cq.cmd(at);
        const pkt = cq.pkt(at);
        const f: BranchCursor | null = pkt
          ? new TreeReader(pkt).read_branch()
          : null;
        if (cq.flags(at) & frame.RES) {
          const waiting = this._waiting.get(tag);
          if (waiting !== undefined) {
            this._waiting.delete(tag);
            waiting(cq.result(at), f);
          } else if (cq.result(at) < 0) {
            console.warn(
              `[xpute/doorbell] command 0x${cmd.toString(16)} failed: ${strerror(
                cq.result(at),
              )}`,
            );
          }
        } else {
          const result = this._listener(tag, cmd, f, pkt);
          if (cq.flags(at) & frame.ACKREQ && result !== undefined) {
            this.complete(tag, cmd, result);
          }
        }
        cq.advance();
        read++;
      }
    } finally {
      this._reading = false;
    }
    // A task of the core may be waiting for the room this read gave back,
    // without having asked for a turn (xpute-core sched/edge.rs
    // `runtime_turn`), so a turn is asked for it.
    if (read > 0) this.on_wake?.(0);
    const later = this._later;
    this._later = [];
    for (const [tag, cmd, flags, result, encode] of later) {
      const errnum = this._submit(tag, cmd, encode, flags, result);
      if (errnum !== Errno.OK) throw new InvariantError(errnum);
    }
  }

  /** Never called from a listener, which could not wait for room, since
   * waiting takes a turn. Returns EMSGSIZE for a packet larger than a
   * slot. */
  private _submit(
    tag: u32,
    cmd: u32,
    encode: Encode | null,
    flags: u32,
    result: i32 = 0,
  ): i32 {
    if (this._reading) throw new InvariantError(Errno.EBUSY);
    if (this._terminated) return Errno.OK;
    const pkt = encode?.(this) ?? null;
    for (;;) {
      const errnum = this._sq.push(tag, cmd, flags, pkt, result);
      if (errnum === Errno.OK || errnum === Errno.EMSGSIZE) return errnum;
      if (errnum !== Errno.EAGAIN) {
        throw new Error(
          `[xpute/doorbell] command 0x${cmd.toString(
            16,
          )} refused by the ring: ${errnum}`,
        );
      }
      this.ring(0);
    }
  }
}
