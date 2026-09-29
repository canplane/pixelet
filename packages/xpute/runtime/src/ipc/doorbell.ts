// @xpute/runtime/ipc/doorbell.ts

/**
 * The host's end of a guest's rings: commands pushed into the submission
 * ring, the guest's doorbell rung with a quota, and the completion ring read
 * dry — replies to the calls waiting on them, signals to whoever hears them.
 *
 * A ring of the doorbell is a whole turn (sched/quantum.ts): the rising edge
 * carries the quota, and the call's return is the falling edge, when the
 * guest wants its next turn. The guest runs only inside a ring. A ring with no
 * quota is a turn all the same: a query asked at boot, or the one message
 * that cannot wait for a frame.
 *
 * Everything a turn leaves is read before the next one is rung (the
 * contract's rule 5, ARCHITECTURE.xpute.md), so the completion ring holds one
 * turn's worth and a listener may not ring the guest while it is being read
 * — that turn would begin with the one before it half read. A submission the
 * guest held back for want of room waits in its ring, and the guest asks for
 * the next turn while one does.
 */

import type { f64, i32, u32 } from "@xpute/core/abi/word.ts";
import type { U8Array } from "@xpute/core/abi/array.ts";
import { Errno } from "@xpute/core/status/errno.spec.ts";
import { type BranchCursor, type BranchView, encoder, TreeReader, TreeView } from "@xpute/core/wire/xtp/mod.ts";
import { FrameFlag } from "./frame.ts";
import { Ring } from "./ring.ts";
import { InvariantError } from "@xpute/core/status/error.ts";

/** A packet of one branch, its children written by `fill`. */
export function packet(fill: (b: BranchView) => void): U8Array {
  return encoder.encode(new TreeView().branch(fill));
}

/** What the guest raised: the message's tag, its number, and its packet's
 * branch, valid only inside the call. */
export type SignalListener = (tag: u32, cmd: u32, f: BranchCursor | null) => void;

/** The guest's doorbell: a turn of `quota_ms`, and when it wants the next. */
export type Interrupt = (quota_ms: f64) => f64;

export class Doorbell {
  private readonly sq: Ring;
  private readonly cq: Ring;
  /** The calls waiting on their replies, by tag. */
  private next_call: u32 = 0;
  private readonly waiting = new Map<u32, (result: i32, f: BranchCursor | null) => void>();
  /** Whether the completion ring is being read: a listener runs inside it. */
  private reading = false;
  /** Who hears the signals. */
  signal: SignalListener | null = null;
  /** Who hears every falling edge's ask, whichever ring it came back from. */
  wake: ((wake_ms: f64) => void) | null = null;

  constructor(mem: ArrayBufferLike, sq_at: u32, cq_at: u32, private readonly interrupt: Interrupt) {
    this.sq = new Ring(mem, sq_at);
    this.cq = new Ring(mem, cq_at);
  }

  /** Sends a command that asks for no reply, applied on the next ring. */
  send(tag: u32, cmd: u32, pkt: U8Array | null = null): void {
    this.submit(tag, cmd, pkt, 0);
  }

  /**
   * A turn of `quota_ms`, and everything it left handed out: the falling
   * edge's ask, as `wake` hears it too.
   */
  ring(quota_ms: f64): f64 {
    if (this.reading) throw new InvariantError(Errno.EBUSY);
    const wake = this.interrupt(quota_ms);
    this.read();
    this.wake?.(wake);
    return wake;
  }

  /**
   * A query answered in the ring it is asked in: submitted with ACKREQ
   * behind whatever waits, and rung with no quota until the reply comes back.
   * `take` reads the reply; its packet is valid only inside it.
   */
  call<T>(cmd: u32, pkt: U8Array | null, take: (result: i32, reply: BranchCursor | null) => T): T {
    // Refused before anything is submitted: a call from inside a listener
    // cannot be answered without a turn, and one left in the ring would be
    // applied after the caller was told it failed.
    if (this.reading) throw new InvariantError(Errno.EBUSY);
    const tag = this.next_call = (this.next_call + 1) >>> 0;
    let out: { value: T } | null = null;
    this.waiting.set(tag, (result, f) => out = { value: take(result, f) });
    try {
      this.submit(tag, cmd, pkt, FrameFlag.ACKREQ);
      while (out === null) {
        const pending = this.sq.len;
        this.ring(0);
        if (out === null && pending === 0) throw new InvariantError(Errno.EPROTO);
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
        const f: BranchCursor | null = pkt ? new TreeReader(pkt).read_branch() : null;
        if (cq.flags(at) & FrameFlag.RES) {
          const waiting = this.waiting.get(tag);
          if (waiting !== undefined) {
            waiting(cq.result(at), f);
          } else if (cq.result(at) < 0) {
            console.warn(`[xpute/doorbell] command 0x${cmd.toString(16)} failed: errno ${cq.result(at)}`);
          }
        } else if (this.signal !== null) {
          this.signal(tag, cmd, f);
        } else {
          console.warn(`[xpute/doorbell] signal 0x${cmd.toString(16)} raised with no one to hear it`);
        }
        cq.advance();
      }
    } finally {
      this.reading = false;
    }
  }

  private submit(tag: u32, cmd: u32, pkt: U8Array | null, flags: u32): void {
    for (;;) {
      const errno = this.sq.push(tag, cmd, flags, pkt);
      if (errno === Errno.OK) return;
      if (errno !== Errno.EAGAIN) throw new Error(`[xpute/doorbell] command 0x${cmd.toString(16)} refused by the ring: ${errno}`);
      this.ring(0);
    }
  }
}
