// @xpute/host/ipc/doorbell.ts

/**
 * The host's end of a guest's rings. One ring of the doorbell is one turn
 * (sched/strobe.ts). Everything a turn leaves is read before the next is rung,
 * so a listener must not ring the guest while the completion ring is read. A
 * frame's tag is the sender's id (ipc/frame.json): a host's call takes the
 * next, which its reply keeps, and what the guest raises is handed to the one
 * listener, which tells the commands apart.
 *
 * A guest on a thread of its own (host.ts `Guest.thread`) is kicked after
 * each submission and after each read of the completion ring, which may have
 * made the room a turn waits for, and runs on its own; what it posts is read
 * as it says it has (`ack`). A ring is then a turn waited for, a call's. A
 * guest that runs only when rung is asked a turn after a read that took
 * frames instead, for what the head passing gave back.
 */

import type { f64, i32, u32 } from "@xpute/kit/abi/word.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { strerror } from "@xpute/kit/status/errno.spec.ts";
import {
  type BranchCursor,
  encoder,
  type NodeValue,
  type NodeView,
  type PacketRoom,
  PacketWriter,
  TreeReader,
  TreeView,
} from "@xpute/kit/wire/xtp/mod.ts";
import { NO_WAKE } from "../sched/wake.ts";
import * as frame from "./frame.ts";
import { Ring } from "./ring.ts";
import { InvariantError } from "@xpute/kit/status/error.ts";

/** Where a stopped guest's packets are written: nowhere, as its sends are dropped. */
const NOWHERE = new Uint8Array(0);
const NOWHERE_VIEW = new DataView(NOWHERE.buffer);

/** What the host hears the guest send; valid only inside the call. A frame
 * the guest calls with is answered with what it returns, 0 for nothing; one
 * it posts with ACKREQ is completed with what it returns, or, for nothing,
 * by its device later (`Doorbell.complete`). */
export type OnEvent = (
  tag: u32,
  cmd: u32,
  f: BranchCursor | null,
) => i32 | void;

/** Writes a command's packet into the room it is given, as a generated
 * `encode_*` does. */
export type Encode = (room: PacketRoom) => PacketWriter | null;

type Run = (quota_ms: f64) => f64;
type Kick = (quota_ms: f64) => void;

export class Doorbell implements PacketRoom {
  private readonly _sq: Ring;
  /** The one writer every packet is written with, in place in its slot. */
  private readonly _writer = new PacketWriter();
  private readonly _cq: Ring;
  private _next_call: u32 = 0;
  private readonly _waiting = new Map<
    u32,
    (result: i32, f: BranchCursor | null) => void
  >();
  /** A listener runs inside it. */
  private _reading = false;
  /** The guest is gone: its turn stopped partway, or the host could not take
   * what it posted, and the submission it was applying would end it again on
   * every ring. */
  private _terminated = false;
  /** ENOSYS for a call until the host listens. */
  private _listener: OnEvent = () => Errno.ENOSYS;
  /** What was sent or completed while the completion ring was read, its
   * tag, command, flags, result and packet, submitted once it is: a packet
   * is written in its slot, and room for one takes a turn. */
  private _later: [u32, u32, u32, i32, Encode | null][] = [];
  wake: ((wake_ms: f64) => void) | null = null;
  /** Told what the guest's own thread ran since it was last heard: its
   * longest turn and when it wants the next. */
  on_turn: ((turn_ms: f64, wake_ms: f64) => void) | null = null;
  /** Told once, as the guest stops. */
  onterminated: (() => void) | null = null;

  private readonly _run: Run;
  /** Wakes a guest on its own thread, none for one that runs only when rung. */
  readonly kick: Kick | null;
  /** Submissions kick nothing: the kick after them does (`batch`). */
  private _batching = false;

  /** The guest is gone, and no turn runs again. */
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

  /** The writer, begun over the next submission's slot for a packet of
   * `count` children, room made for it first; valid until that submission. */
  writer(cnt: u32): PacketWriter {
    if (this._reading) throw new InvariantError(Errno.EBUSY);
    while (!this._terminated && !this._sq.write(this._writer, cnt)) this.ring(0);
    if (this._terminated) this._writer.begin(NOWHERE, NOWHERE_VIEW, 0, 0, cnt);
    return this._writer;
  }

  /** Whether a command carrying `pkt` can be sent at all; one that cannot is
   * the caller's input to refuse, at the boundary it came through. */
  fits(pkt: PacketWriter): boolean {
    return pkt.finish() >= 0;
  }

  /** What the guest sends or calls with is handed to `listener`. */
  listen(listener: OnEvent): void {
    this._listener = listener;
  }

  /** Tagged 0; while the completion ring is read, once it is. */
  send(cmd: u32, encode: Encode | null = null): void {
    if (this._reading) {
      this._later.push([0, cmd, 0, 0, encode]);
      return;
    }
    const errnum = this._submit(0, cmd, encode, 0);
    if (errnum !== Errno.OK) throw new InvariantError(errnum);
  }

  /** The guest's request tagged `tag`, posted with ACKREQ, completed: a frame
   * of its tag and command flagged RES, its result `result` and its packet
   * `encode`'s; while the completion ring is read, once it is. */
  complete(tag: u32, cmd: u32, result: i32, encode: Encode | null = null): void {
    if (this._reading) {
      this._later.push([tag, cmd, frame.RES, result, encode]);
      return;
    }
    const errnum = this._submit(tag, cmd, encode, frame.RES, result);
    if (errnum !== Errno.OK) throw new InvariantError(errnum);
  }

  /** `cmd` sent and rung, its arguments its packet's fields in turn, each
   * lowered as XTP lowers a value (wire/xtp `TreeView.set`): a command from
   * outside the host's own code, a console's. */
  cmd(cmd: u32, ...args: NodeValue<NodeView>[]): void {
    this.send(cmd, () => this.lower(args));
    this.ring(0);
  }

  /** The next submission's packet, `args` its fields in turn, each lowered as
   * XTP lowers a value (wire/xtp `TreeView.set`); none for no arguments. */
  lower(args: NodeValue<NodeView>[]): PacketWriter | null {
    if (args.length === 0) return null;
    const pkt = this.writer(args.length);
    // Each is grafted into the slot, so a slot is its most.
    const cap = { init_cap: this._sq.slot_bytes, max_cap: this._sq.slot_bytes };
    for (const a of args) {
      pkt.graft(encoder.encode(new TreeView().set(a), cap));
    }
    return pkt;
  }

  ring(quota_ms: f64): f64 {
    if (this._reading) throw new InvariantError(Errno.EBUSY);
    if (this._terminated) return NO_WAKE;
    let wake: f64;
    try {
      wake = this._run(quota_ms);
    } catch (err) {
      // What it posted before it trapped, its report among it, is read.
      this._terminate("the guest trapped", err, true);
      return NO_WAKE;
    }
    try {
      this._read();
    } catch (err) {
      // A frame the host could not take: the guest broke the protocol, or
      // what answers it failed, and nothing after that frame can be read.
      this._terminate(
        "the host could not take what the guest posted",
        err,
        false,
      );
      return NO_WAKE;
    }
    this.wake?.(wake);
    return wake;
  }

  /** `f`'s submissions kick nothing, so the kick its caller makes after it
   * gives the turn that applies them its quota. */
  batch(f: () => void): void {
    const was = this._batching;
    this._batching = true;
    try {
      f();
    } finally {
      this._batching = was;
    }
  }

  /** The guest's own thread has posted or asked since it was last heard:
   * the completion ring read dry, and what it asked told, or, where it has
   * stopped (`wake_ms` NaN), what it posted read and its stop told. */
  ack(wake_ms: f64, turn_ms: f64): void {
    if (this._terminated) return;
    if (Number.isNaN(wake_ms)) {
      this._terminate("the guest stopped", null, true);
      return;
    }
    try {
      this._read();
    } catch (err) {
      this._terminate(
        "the host could not take what the guest posted",
        err,
        false,
      );
      return;
    }
    this.on_turn?.(turn_ms, wake_ms);
    this.wake?.(wake_ms);
  }

  /** The guest is gone: printed here, once, and told, since a turn's caller is
   * a timer or an event with nothing to do about it. `read` first reads what
   * it posted. */
  private _terminate(why: string, err: unknown, read: boolean): void {
    console.error(`[xpute/doorbell] ${why}:`, err);
    this._terminated = true;
    try {
      if (read) this._read();
    } catch (err) {
      console.error(
        "[xpute/doorbell] the host could not take what the guest posted:",
        err,
      );
    } finally {
      this.onterminated?.();
    }
  }

  /** Submitted with ACKREQ, tagged with the next call's id, and rung with no quota until the
   * reply returns; the reply is valid only inside `read`. A guest that has
   * stopped answers ENOTRECOVERABLE, and a packet past a slot EMSGSIZE, with
   * no reply. Refused while the completion ring is read, since the answer
   * takes a turn. */
  call<T>(
    cmd: u32,
    encode: Encode | null,
    read: (result: i32, reply: BranchCursor | null) => T,
  ): T {
    if (this._reading) throw new InvariantError(Errno.EBUSY);
    if (this._terminated) return read(Errno.ENOTRECOVERABLE, null);
    // 0 is a send's.
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

  /** The guest's call, a frame it laid at `at` in the completion ring and has
   * not published, answered by the listener before the guest goes on. Run
   * inside the guest's turn, so the answer rings nothing. */
  answer(at: u32): i32 {
    const cq = this._cq;
    const e = cq.entry_of(at);
    const pkt = cq.pkt(e);
    const f: BranchCursor | null = pkt
      ? new TreeReader(pkt).read_branch()
      : null;
    this._reading = true;
    try {
      return this._listener(cq.tag(e), cq.cmd(e), f) ?? 0;
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
            waiting(cq.result(at), f);
          } else if (cq.result(at) < 0) {
            console.warn(
              `[xpute/doorbell] command 0x${cmd.toString(16)} failed: ${strerror(
                cq.result(at),
              )}`,
            );
          }
        } else {
          const result = this._listener(tag, cmd, f);
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
    // A guest task may wait on what the head passing gave back with no turn
    // asked (xpute-guest sched/edge.rs `host_turn`).
    if (read > 0) {
      if (this.kick) this.kick(-1);
      else this.wake?.(0);
    }
    const later = this._later;
    this._later = [];
    for (const [tag, cmd, flags, result, encode] of later) {
      const errnum = this._submit(tag, cmd, encode, flags, result);
      if (errnum !== Errno.OK) throw new InvariantError(errnum);
    }
  }

  /** Never from a listener: one could not wait for room, since that takes a
   * turn. EMSGSIZE where the packet is past a slot. */
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
      if (errnum === Errno.OK && !this._batching) this.kick?.(-1);
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
