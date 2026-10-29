// @xpute/host/ipc/doorbell.ts

/**
 * The host's end of a guest's rings. One ring of the doorbell is one turn
 * (sched/quantum.ts). Everything a turn leaves is read before the next is rung,
 * so a listener must not ring the guest while the completion ring is read.
 */

import type { f64, i32, u32 } from "@xpute/kit/abi/word.ts";
import { Errno, strerror } from "@xpute/kit/status/errno.spec.ts";
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
import { NO_WAKE } from "../sched/quantum.ts";
import * as frame from "./frame.ts";
import { Ring } from "./ring.ts";
import { InvariantError } from "@xpute/kit/status/error.ts";

/** Where a trapped guest's packets are written: nowhere, as its sends are dropped. */
const NOWHERE = new Uint8Array(0);
const NOWHERE_VIEW = new DataView(NOWHERE.buffer);

/** Valid only inside the call. */
type SignalListener = (tag: u32, cmd: u32, f: BranchCursor | null) => void;

type Intr = (quota_ms: f64) => f64;

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
  signal: SignalListener | null = null;
  wake: ((wake_ms: f64) => void) | null = null;
  /** Told once, as the guest traps. */
  stopped: (() => void) | null = null;

  constructor(
    mem: ArrayBufferLike,
    sq_at: u32,
    cq_at: u32,
    private readonly intr: Intr,
  ) {
    this.sq = new Ring(mem, sq_at);
    this.cq = new Ring(mem, cq_at);
  }

  /** The writer, begun over the next submission's slot for a packet of
   * `count` children, room made for it first; valid until that submission. */
  writer(count: u32): PacketWriter {
    if (this.reading) throw new InvariantError(Errno.EBUSY);
    while (!this.trapped && !this.sq.write(this.writing, count)) this.ring(0);
    if (this.trapped) this.writing.begin(NOWHERE, NOWHERE_VIEW, 0, 0, count);
    return this.writing;
  }

  /** Whether a command carrying `pkt` can be sent at all; one that cannot is
   * the caller's input to refuse, at the boundary it came through. */
  fits(pkt: PacketWriter): boolean {
    return pkt.finish() >= 0;
  }

  send(tag: u32, cmd: u32, pkt: PacketWriter | null = null): void {
    this.submit(tag, cmd, pkt, 0);
  }

  /** `cmd` sent and rung, its arguments its packet's fields in turn, each
   * lowered as XTP lowers a value (wire/xtp `TreeView.set`): a command from
   * outside the host's own code, a console's. */
  cmd(cmd: u32, ...args: NodeValue<NodeView>[]): void {
    const pkt = args.length === 0 ? null : this.writer(args.length);
    // Each is grafted into the slot, so a slot is its most.
    const cap = { init_cap: this.sq.slot_bytes, max_cap: this.sq.slot_bytes };
    for (const a of args) {
      pkt?.graft(encoder.encode(new TreeView().set(a), cap));
    }
    this.send(0, cmd, pkt);
    this.ring(0);
  }

  ring(quota_ms: f64): f64 {
    if (this.reading) throw new InvariantError(Errno.EBUSY);
    if (this.trapped) return NO_WAKE;
    let wake: f64;
    try {
      wake = this.intr(quota_ms);
    } catch (err) {
      // Printed here, once: what a caller catches next is only the stop.
      console.error("[xpute/doorbell] the guest trapped:", err);
      this.trapped = true;
      this.stopped?.();
      throw err;
    }
    this.read();
    this.wake?.(wake);
    return wake;
  }

  /** Submitted with ACKREQ and rung with no quota until the reply returns; the reply is valid only inside `take`. */
  call<T>(
    cmd: u32,
    pkt: PacketWriter | null,
    take: (result: i32, reply: BranchCursor | null) => T,
  ): T {
    if (this.trapped) throw new InvariantError(Errno.ENOTRECOVERABLE);
    const tag = (this.next_call = (this.next_call + 1) >>> 0);
    let out: { val: T } | null = null;
    this.waiting.set(tag, (result, f) => (out = { val: take(result, f) }));
    try {
      this.submit(tag, cmd, pkt, frame.ACKREQ);
      while (out === null) {
        const pending = this.sq.len;
        this.ring(0);
        if (out === null && pending === 0) {
          throw new InvariantError(Errno.EPROTO);
        }
      }
    } finally {
      this.waiting.delete(tag);
    }
    return (out as { value: T }).value;
  }

  private read(): void {
    this.reading = true;
    try {
      const cq = this.cq;
      for (let at = cq.peek(); at >= 0; at = cq.peek()) {
        const tag = cq.tag(at);
        const cmd = cq.cmd(at);
        const pkt = cq.packet(at);
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
        } else if (this.signal !== null) {
          this.signal(tag, cmd, f);
        } else {
          console.warn(
            `[xpute/doorbell] signal 0x${cmd.toString(
              16,
            )} raised with no one to hear it`,
          );
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
