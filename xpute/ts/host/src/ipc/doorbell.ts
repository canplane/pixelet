// @xpute/host/ipc/doorbell.ts

/**
 * The host's end of a guest's rings. One ring of the doorbell is one turn
 * (sched/strobe.ts). Everything a turn leaves is read before the next is rung,
 * so a listener must not ring the guest while the completion ring is read. A
 * frame's tag names a port (ipc/frame.json): the doorbell sends as the port it
 * is given, and hands what the guest raises to the port it names, where one
 * listens.
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

/** Where a trapped guest's packets are written: nowhere, as its sends are dropped. */
const NOWHERE = new Uint8Array(0);
const NOWHERE_VIEW = new DataView(NOWHERE.buffer);

/** What a port hears the guest send; valid only inside the call. A frame the
 * guest calls with is answered with what it returns, 0 for nothing. */
export type OnEvent = (
  tag: u32,
  cmd: u32,
  f: BranchCursor | null,
) => i32 | void;

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
  /** A trapped guest is gone: its turn stopped partway, and the submission
   * it was applying would trap it again on every ring. */
  private trapped = false;
  private readonly hearing = new Map<u32, OnEvent>();
  wake: ((wake_ms: f64) => void) | null = null;
  /** Told once, as the guest traps; the mount's (host.ts), which halts the strobe. */
  stopped: (() => void) | null = null;

  private readonly run: Run;

  constructor(mem: ArrayBufferLike, sq_at: u32, cq_at: u32, run: Run) {
    this.run = run;
    this.sq = new Ring(mem, sq_at);
    this.cq = new Ring(mem, cq_at);
  }

  /** The writer, begun over the next submission's slot for a packet of
   * `count` children, room made for it first; valid until that submission. */
  writer(cnt: u32): PacketWriter {
    if (this.reading) throw new InvariantError(Errno.EBUSY);
    while (!this.trapped && !this.sq.write(this.writing, cnt)) this.ring(0);
    if (this.trapped) this.writing.begin(NOWHERE, NOWHERE_VIEW, 0, 0, cnt);
    return this.writing;
  }

  /** Whether a command carrying `pkt` can be sent at all; one that cannot is
   * the caller's input to refuse, at the boundary it came through. */
  fits(pkt: PacketWriter): boolean {
    return pkt.finish() >= 0;
  }

  /** What the guest sends or calls `port` with is handed to `heard`; what it
   * sends a port none listens on is dropped, and a call to one ENOSYS. */
  on(port: u32, heard: OnEvent): void {
    this.hearing.set(port, heard);
  }

  /** As the port `src`. */
  send(src: u32, cmd: u32, pkt: PacketWriter | null = null): void {
    this.submit(frame.tag_of(src, 0), cmd, pkt, 0);
  }

  /** `cmd` sent as the port `src` and rung, its arguments its packet's fields
   * in turn, each lowered as XTP lowers a value (wire/xtp `TreeView.set`): a
   * command from outside the host's own code, a console's. */
  cmd(src: u32, cmd: u32, ...args: NodeValue<NodeView>[]): void {
    this.send(src, cmd, this.lower(args));
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
    if (this.trapped) return NO_WAKE;
    let wake: f64;
    try {
      wake = this.run(quota_ms);
    } catch (err) {
      // What it posted before it trapped, its report among it, is read.
      this.stop("the guest trapped", err, true);
      return NO_WAKE;
    }
    try {
      this.read();
    } catch (err) {
      // A frame its port could not take: the guest broke the protocol, or
      // what answers it failed, and nothing after that frame can be read.
      this.stop("a port could not take what the guest posted", err, false);
      return NO_WAKE;
    }
    this.wake?.(wake);
    return wake;
  }

  /** The guest is gone: printed here, once, and told, since a turn's caller is
   * a timer or an event with nothing to do about it. `read` first reads what
   * it posted. */
  private stop(why: string, err: unknown, read: boolean): void {
    console.error(`[xpute/doorbell] ${why}:`, err);
    this.trapped = true;
    try {
      if (read) this.read();
    } catch (err) {
      console.error(
        "[xpute/doorbell] a port could not take what the guest posted:",
        err,
      );
    } finally {
      this.stopped?.();
    }
  }

  /** Submitted as the port `src` with ACKREQ and rung with no quota until the
   * reply returns; the reply is valid only inside `take`. A guest that has
   * trapped answers ENOTRECOVERABLE, with no reply. */
  call<T>(
    src: u32,
    cmd: u32,
    pkt: PacketWriter | null,
    take: (result: i32, reply: BranchCursor | null) => T,
  ): T {
    if (this.trapped) return take(Errno.ENOTRECOVERABLE, null);
    this.next_call = frame.id_of(this.next_call + 1);
    const tag = frame.tag_of(src, this.next_call);
    let out: { val: T } | null = null;
    this.waiting.set(tag, (result, f) => (out = { val: take(result, f) }));
    try {
      this.submit(tag, cmd, pkt, frame.ACKREQ);
      while (out === null) {
        const pending = this.sq.len;
        this.ring(0);
        if (out === null && this.trapped) {
          return take(Errno.ENOTRECOVERABLE, null);
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
   * not published, answered by the port its tag names, before the guest goes
   * on. Run inside the guest's turn, so the answer rings nothing. */
  answer(at: u32): i32 {
    const cq = this.cq;
    const e = cq.entry_of(at);
    const tag = cq.tag(e);
    const heard = this.hearing.get(frame.port_of(tag));
    if (!heard) return Errno.ENOSYS;
    const pkt = cq.pkt(e);
    const f: BranchCursor | null = pkt
      ? new TreeReader(pkt).read_branch()
      : null;
    this.reading = true;
    try {
      return heard(tag, cq.cmd(e), f) ?? 0;
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
              `[xpute/doorbell] port ${frame.port_of(tag)}'s command 0x${cmd.toString(
                16,
              )} failed: ${strerror(cq.result(at))}`,
            );
          }
        } else {
          this.hearing.get(frame.port_of(tag))?.(tag, cmd, f);
        }
        cq.advance();
      }
    } finally {
      this.reading = false;
    }
  }

  /** Never from a listener: one could not wait for room, since that takes a turn. */
  private submit(
    tag: u32,
    cmd: u32,
    pkt: PacketWriter | null,
    flags: u32,
  ): void {
    if (this.reading) throw new InvariantError(Errno.EBUSY);
    if (this.trapped) return;
    for (;;) {
      const errnum = this.sq.push(tag, cmd, flags, pkt);
      if (errnum === Errno.OK) return;
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
