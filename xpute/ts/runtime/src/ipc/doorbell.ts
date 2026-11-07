// @xpute/runtime/ipc/doorbell.ts

/**
 * The runtime's end of a core's rings. One ring of the doorbell is one turn
 * (sched/scheduler.ts). Everything a turn leaves is read before the next is rung,
 * so a listener must not ring the core while the completion ring is read. A
 * frame's tag is the sender's id (ipc/frame.json): a runtime's call takes the
 * next, which its reply keeps, and what the core raises is handed to the one
 * listener, which tells the commands apart.
 *
 * A core on a thread of its own (runtime.ts `Core.thread`) is kicked after
 * each submission and after each read of the completion ring, which may have
 * made the room a turn waits for, and runs on its own; what it posts is read
 * as it says it has (`turns`). A ring is then a turn waited for, a call's. A
 * core that runs only when rung is asked a turn after a read that took
 * frames instead, for what the head passing gave back.
 */

import type { f64, i32, u32 } from "@xpute/kit/abi/word.ts";
import type { U8Array } from "@xpute/kit/abi/array.ts";
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

/** Where a stopped core's packets are written: nowhere, as its sends are dropped. */
const NOWHERE = new Uint8Array(0);
const NOWHERE_VIEW = new DataView(NOWHERE.buffer);

/** What the runtime hears the core send; `f` and `pkt`, the packet read and
 * its bytes, are valid only inside the call. A frame the core calls with is
 * answered with what it returns, 0 for nothing; one it posts with ACKREQ is
 * completed with what it returns, or, for nothing, by its device later
 * (`Doorbell.complete`). */
export type OnImport = (
  tag: u32,
  cmd: u32,
  f: BranchCursor | null,
  pkt: U8Array | null,
) => i32 | void;

/** Writes a command's packet into the room it is given, as a generated
 * `encode_*` does. */
export type Encode = (room: PacketRoom) => PacketWriter | null;

type Run = (deadline_ms: f64) => f64;
type Kick = (deadline_ms: f64) => void;

/** A packet whose fields are `args` in turn, each lowered as XTP lowers a
 * value (wire/xtp `TreeView.set`), each grafted into a slot of `slot_bytes`;
 * none for no arguments: what a console sends. */
export function encode_args(
  args: NodeValue<NodeView>[],
  slot_bytes: u32,
): Encode {
  return (room) => {
    if (args.length === 0) return null;
    const pkt = room.writer(args.length);
    const cap = { init_cap: slot_bytes, max_cap: slot_bytes };
    for (const a of args) {
      pkt.graft(encoder.encode(new TreeView().set(a), cap));
    }
    return pkt;
  };
}

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
  /** The core is gone: its turn stopped partway, or the runtime could not take
   * what it posted, and the submission it was applying would end it again on
   * every ring. */
  private _terminated = false;
  /** ENOSYS for a call until the runtime listens. */
  private _listener: OnImport = () => Errno.ENOSYS;
  /** What was sent or completed while the completion ring was read, its
   * tag, command, flags, result and packet, submitted once it is: a packet
   * is written in its slot, and room for one takes a turn. */
  private _later: [u32, u32, u32, i32, Encode | null][] = [];
  on_wake: ((wake_ms: f64) => void) | null = null;
  /** Told of every turn heard, before what it posted is read and `on_wake`
   * told the same wake, so what that read asks outlives the turn: what the
   * core's own thread ran since it was last heard, whoever kicked it
   * (`turns`), and each turn rung (`ring`): its longest turn and when it
   * wants the next. */
  on_turn: ((turn_ms: f64, wake_ms: f64) => void) | null = null;
  /** Told once, as the core stops. */
  onterminated: (() => void) | null = null;

  private readonly _run: Run;
  /** Wakes a core on its own thread, none for one that runs only when rung. */
  readonly kick: Kick | null;
  /** Submissions kick nothing: the kick after them does (`batch`). */
  private _batching = false;

  /** The core is gone, and no turn runs again. */
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
    while (!this._terminated && !this._sq.write(this._writer, cnt))
      this.ring(0);
    if (this._terminated) this._writer.begin(NOWHERE, NOWHERE_VIEW, 0, 0, cnt);
    return this._writer;
  }

  /** Whether a command carrying `pkt` can be sent at all; one that cannot is
   * the caller's input to refuse, at the boundary it came through. */
  fits(pkt: PacketWriter): boolean {
    return pkt.finish() >= 0;
  }

  /** What the core sends or calls with is handed to `listener`. */
  listen(listener: OnImport): void {
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

  /** Posted with ACKREQ, tagged with the next call's id, and answered by the
   * reply the core posts as it applies it, read with the completion ring:
   * nothing waits for it here. A core that has stopped answers
   * ENOTRECOVERABLE, and every request still waiting is answered so as it
   * stops; a packet past a slot is EMSGSIZE. While the completion ring is
   * read, it is posted once it is. */
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

  /** The core's request tagged `tag`, posted with ACKREQ, completed: a frame
   * of its tag and command flagged RES, its result `result` and its packet
   * `encode`'s; while the completion ring is read, once it is. */
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

  /** `cmd` sent and rung, its arguments its packet's fields in turn
   * (`encode_args`): a command from outside the runtime's own code, a
   * console's. */
  cmd(cmd: u32, ...args: NodeValue<NodeView>[]): void {
    this.send(cmd, encode_args(args, this._sq.slot_bytes));
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
      // What it posted before it trapped, its report among it, is read.
      this._terminate("the core trapped", err, true);
      return NO_WAKE;
    }
    this.on_turn?.(performance.now() - t0, wake);
    try {
      this._read();
    } catch (err) {
      // A frame the runtime could not take: the core broke the protocol, or
      // what answers it failed, and nothing after that frame can be read.
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

  /** `f`'s submissions kick nothing, so the kick its caller makes after it
   * gives the turn that applies them its deadline. */
  batch(f: () => void): void {
    const was = this._batching;
    this._batching = true;
    try {
      f();
    } finally {
      this._batching = was;
    }
  }

  /** The core's own thread has posted or asked since it was last heard:
   * the completion ring read dry, and what it asked told, or, where it has
   * stopped (`wake_ms` NaN), what it posted read and its stop told. */
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

  /** The core is gone: printed here, once, and told, since a turn's caller is
   * a timer or an import's handler with nothing to do about it. `read` first reads what
   * it posted. */
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

  /** Submitted with ACKREQ, tagged with the next call's id, and rung with a deadline of 0 until the
   * reply returns; the reply is valid only inside `read`. A core that has
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

  /** The core's call, a frame it laid at `at` in the completion ring and has
   * not published, answered by the listener before the core goes on. Run
   * inside the core's turn, so the answer rings nothing. */
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
    // A core task may wait on what the head passing gave back with no turn
    // asked (xpute-core sched/edge.rs `runtime_turn`).
    if (read > 0) {
      if (this.kick) this.kick(-1);
      else this.on_wake?.(0);
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
