// @xpute/host/ipc/doorbell.ts

/**
 * The host's end of a guest's rings. One ring of the doorbell is one turn
 * (sched/strobe.ts). Everything a turn leaves is read before the next is rung,
 * so a listener must not ring the guest while the completion ring is read. A
 * frame's tag is the sender's id (ipc/frame.json): a host's call takes the
 * next, which its reply keeps, and what the guest raises is handed to the one
 * listener, which tells the commands apart.
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
 * the guest calls with is answered with what it returns, 0 for nothing. */
export type OnEvent = (
  tag: u32,
  cmd: u32,
  f: BranchCursor | null,
) => i32 | void;

/** Writes a command's packet into the room it is given, as a generated
 * `encode_*` does. */
export type Encode = (room: PacketRoom) => PacketWriter | null;

type Run = (quota_ms: f64) => f64;

export class Doorbell implements PacketRoom {
  private readonly sq: Ring;
  /** The one writer every packet is written with, in place in its slot. */
  private readonly writing = new PacketWriter();
  private readonly cq: Ring;
  private next_call: u32 = 0;
  private readonly waiting = new Map<
    u32,
    (result: i32, f: BranchCursor | null) => void
  >();
  /** A listener runs inside it. */
  private reading = false;
  /** The guest is gone: its turn stopped partway, or the host could not take
   * what it posted, and the submission it was applying would end it again on
   * every ring. */
  #terminated = false;
  /** ENOSYS for a call until the host listens. */
  private heard: OnEvent = () => Errno.ENOSYS;
  /** What was sent while the completion ring was read, sent once it is: a
   * packet is written in its slot, and room for one takes a turn. */
  private later: [u32, Encode | null][] = [];
  wake: ((wake_ms: f64) => void) | null = null;
  /** Told once, as the guest stops. */
  onterminated: (() => void) | null = null;

  private readonly run: Run;

  /** The guest is gone, and no turn runs again. */
  get terminated(): boolean {
    return this.#terminated;
  }

  constructor(mem: ArrayBufferLike, sq_at: u32, cq_at: u32, run: Run) {
    this.run = run;
    this.sq = new Ring(mem, sq_at);
    this.cq = new Ring(mem, cq_at);
  }

  /** The writer, begun over the next submission's slot for a packet of
   * `count` children, room made for it first; valid until that submission. */
  writer(cnt: u32): PacketWriter {
    if (this.reading) throw new InvariantError(Errno.EBUSY);
    while (!this.#terminated && !this.sq.write(this.writing, cnt)) this.ring(0);
    if (this.#terminated) this.writing.begin(NOWHERE, NOWHERE_VIEW, 0, 0, cnt);
    return this.writing;
  }

  /** Whether a command carrying `pkt` can be sent at all; one that cannot is
   * the caller's input to refuse, at the boundary it came through. */
  fits(pkt: PacketWriter): boolean {
    return pkt.finish() >= 0;
  }

  /** What the guest sends or calls with is handed to `heard`. */
  hear(heard: OnEvent): void {
    this.heard = heard;
  }

  /** Tagged 0; while the completion ring is read, once it is. */
  send(cmd: u32, encode: Encode | null = null): void {
    if (this.reading) {
      this.later.push([cmd, encode]);
      return;
    }
    const errnum = this.submit(0, cmd, encode, 0);
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
    const cap = { init_cap: this.sq.slot_bytes, max_cap: this.sq.slot_bytes };
    for (const a of args) {
      pkt.graft(encoder.encode(new TreeView().set(a), cap));
    }
    return pkt;
  }

  ring(quota_ms: f64): f64 {
    if (this.reading) throw new InvariantError(Errno.EBUSY);
    if (this.#terminated) return NO_WAKE;
    let wake: f64;
    try {
      wake = this.run(quota_ms);
    } catch (err) {
      // What it posted before it trapped, its report among it, is read.
      this.terminate("the guest trapped", err, true);
      return NO_WAKE;
    }
    try {
      this.read();
    } catch (err) {
      // A frame the host could not take: the guest broke the protocol, or
      // what answers it failed, and nothing after that frame can be read.
      this.terminate(
        "the host could not take what the guest posted",
        err,
        false,
      );
      return NO_WAKE;
    }
    this.wake?.(wake);
    return wake;
  }

  /** The guest is gone: printed here, once, and told, since a turn's caller is
   * a timer or an event with nothing to do about it. `read` first reads what
   * it posted. */
  private terminate(why: string, err: unknown, read: boolean): void {
    console.error(`[xpute/doorbell] ${why}:`, err);
    this.#terminated = true;
    try {
      if (read) this.read();
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
    if (this.reading) throw new InvariantError(Errno.EBUSY);
    if (this.#terminated) return read(Errno.ENOTRECOVERABLE, null);
    // 0 is a send's.
    this.next_call = (this.next_call + 1) >>> 0 || 1;
    const tag = this.next_call;
    let out: { val: T } | null = null;
    this.waiting.set(tag, (result, f) => (out = { val: read(result, f) }));
    try {
      const errnum = this.submit(tag, cmd, encode, frame.ACKREQ);
      if (errnum !== Errno.OK) return read(errnum, null);
      while (out === null) {
        const pending = this.sq.len;
        this.ring(0);
        if (out === null && this.#terminated) {
          return read(Errno.ENOTRECOVERABLE, null);
        }
        if (out === null && pending === 0) {
          throw new InvariantError(Errno.EPROTO);
        }
      }
    } finally {
      this.waiting.delete(tag);
    }
    return (out as { val: T }).val;
  }

  /** The guest's call, a frame it laid at `at` in the completion ring and has
   * not published, answered by the listener before the guest goes on. Run
   * inside the guest's turn, so the answer rings nothing. */
  answer(at: u32): i32 {
    const cq = this.cq;
    const e = cq.entry_of(at);
    const pkt = cq.pkt(e);
    const f: BranchCursor | null = pkt
      ? new TreeReader(pkt).read_branch()
      : null;
    this.reading = true;
    try {
      return this.heard(cq.tag(e), cq.cmd(e), f) ?? 0;
    } finally {
      this.reading = false;
    }
  }

  private read(): void {
    this.reading = true;
    try {
      const cq = this.cq;
      for (let at = cq.peek(); at >= 0; at = cq.peek()) {
        const tag = cq.tag(at);
        const cmd = cq.cmd(at);
        const pkt = cq.pkt(at);
        const f: BranchCursor | null = pkt
          ? new TreeReader(pkt).read_branch()
          : null;
        if (cq.flags(at) & frame.RES) {
          const waiting = this.waiting.get(tag);
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
          this.heard(tag, cmd, f);
        }
        cq.advance();
      }
    } finally {
      this.reading = false;
    }
    const later = this.later;
    this.later = [];
    for (const [cmd, encode] of later) this.send(cmd, encode);
  }

  /** Never from a listener: one could not wait for room, since that takes a
   * turn. EMSGSIZE where the packet is past a slot. */
  private submit(tag: u32, cmd: u32, encode: Encode | null, flags: u32): i32 {
    if (this.reading) throw new InvariantError(Errno.EBUSY);
    if (this.#terminated) return Errno.OK;
    const pkt = encode?.(this) ?? null;
    for (;;) {
      const errnum = this.sq.push(tag, cmd, flags, pkt);
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
