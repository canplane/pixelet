// @xpute/host/ipc/doorbell.ts

/**
 * The host's end of a guest's rings. One ring of the doorbell is one turn
 * (sched/quantum.ts). Everything a turn leaves is read before the next is rung,
 * so a listener must not ring the guest while the completion ring is read.
 */

import type { f64, i32, u32 } from "@xpute/kit/abi/word.ts";
import type { U8Array } from "@xpute/kit/abi/array.ts";
import { Errno } from "@xpute/kit/status/errno.spec.ts";
import { type BranchCursor, type BranchView, encoder, TreeReader, TreeView } from "@xpute/kit/wire/xtp/mod.ts";
import { NO_WAKE } from "../sched/quantum.ts";
import * as frame from "./frame.ts";
import { Ring } from "./ring.ts";
import { InvariantError } from "@xpute/kit/status/error.ts";

export function packet(fill: (b: BranchView) => void): U8Array {
  return encoder.encode(new TreeView().branch(fill));
}

/** Valid only inside the call. */
type SignalListener = (tag: u32, cmd: u32, f: BranchCursor | null) => void;

type Interrupt = (quota_ms: f64) => f64;

export class Doorbell {
  private readonly sq: Ring;
  private readonly cq: Ring;
  private next_call: u32 = 0;
  private readonly waiting = new Map<u32, (result: i32, f: BranchCursor | null) => void>();
  /** A listener runs inside it. */
  private reading = false;
  /** A trapped guest is gone: its turn stopped partway, and the submission
   * it was applying would trap it again on every ring. */
  private trapped = false;
  signal: SignalListener | null = null;
  wake: ((wake_ms: f64) => void) | null = null;

  constructor(mem: ArrayBufferLike, sq_at: u32, cq_at: u32, private readonly interrupt: Interrupt) {
    this.sq = new Ring(mem, sq_at);
    this.cq = new Ring(mem, cq_at);
  }

  /** Whether a command carrying `pkt` can be sent at all; one that cannot is
   * the caller's input to refuse, at the boundary it came through. */
  fits(pkt: U8Array): boolean {
    return this.sq.fits(pkt);
  }

  send(tag: u32, cmd: u32, pkt: U8Array | null = null): void {
    this.submit(tag, cmd, pkt, 0);
  }

  ring(quota_ms: f64): f64 {
    if (this.reading) throw new InvariantError(Errno.EBUSY);
    if (this.trapped) return NO_WAKE;
    let wake: f64;
    try {
      wake = this.interrupt(quota_ms);
    } catch (err) {
      this.trapped = true;
      throw err;
    }
    this.read();
    this.wake?.(wake);
    return wake;
  }

  /** Submitted with ACKREQ and rung with no quota until the reply returns; the reply is valid only inside `take`. */
  call<T>(cmd: u32, pkt: U8Array | null, take: (result: i32, reply: BranchCursor | null) => T): T {
    if (this.trapped) throw new InvariantError(Errno.ENOTRECOVERABLE);
    const tag = this.next_call = (this.next_call + 1) >>> 0;
    let out: { value: T } | null = null;
    this.waiting.set(tag, (result, f) => out = { value: take(result, f) });
    try {
      this.submit(tag, cmd, pkt, frame.ACKREQ);
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
        if (cq.flags(at) & frame.RES) {
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

  /** Never from a listener: one could not wait for room, since that takes a turn. */
  private submit(tag: u32, cmd: u32, pkt: U8Array | null, flags: u32): void {
    if (this.reading) throw new InvariantError(Errno.EBUSY);
    if (this.trapped) return;
    for (;;) {
      const errno = this.sq.push(tag, cmd, flags, pkt);
      if (errno === Errno.OK) return;
      if (errno !== Errno.EAGAIN) throw new Error(`[xpute/doorbell] command 0x${cmd.toString(16)} refused by the ring: ${errno}`);
      this.ring(0);
    }
  }
}
