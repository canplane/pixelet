// @xpute/runtime/ipc/doorbell.ts

/**
 * The host's end of a guest's rings: commands pushed into the submission
 * ring, the guest's doorbell rung with a quota, and the completion ring read
 * dry — replies to the calls waiting on them, signals to whoever hears them.
 *
 * A ring of the doorbell is a whole turn (sched/quantum.ts): the rising edge
 * carries the quota, and the call's return is the falling edge, when the
 * guest wants its next turn. The guest runs only inside a ring. A ring with no
 * quota only applies what waits: a query asked at boot, or the one message
 * that cannot wait for a frame.
 *
 * A completion the ring had no room for waits in the guest, and so does a
 * submission it held back for want of room: a turn whose completion ring came
 * back full is read and rung again with no quota until it does not.
 */

import type { f64, i32, u32 } from "@xpute/core/abi/word.ts";
import type { U8Array } from "@xpute/core/abi/array.ts";
import { Errno } from "@xpute/core/status/errno.spec.ts";
import { type BranchCursor, type BranchView, encoder, TreeReader, TreeView } from "@xpute/core/wire/xtp/mod.ts";
import { FrameFlag } from "./frame.ts";
import { Ring } from "./ring.ts";

/** A packet of one branch, its children written by `fill`. */
export function packet(fill: (b: BranchView) => void): U8Array {
  return encoder.encode(new TreeView().branch(fill));
}

/** What the guest raised: the message's tag, its number, and its packet's
 * branch, valid only inside the call. */
export type SignalListener = (tag: u32, cmd: u32, f: BranchCursor | null) => void;

/** The guest's doorbell: a turn of `quota_ms`, and when it wants the next. */
export type Interrupt = (quota_ms: f64) => f64;

/** The sooner of two falling edges' asks, a negative one being none. */
function sooner(a: f64, b: f64): f64 {
  if (a < 0) return b;
  if (b < 0) return a;
  return Math.min(a, b);
}

export class Doorbell {
  private readonly sq: Ring;
  private readonly cq: Ring;
  /** The calls waiting on their replies, by tag; and the replies a call
   * made inside a listener read ahead of the completion ring's head, which
   * the read under it then passes over. */
  private next_call: u32 = 0;
  private readonly waiting = new Map<u32, (result: i32, f: BranchCursor | null) => void>();
  private readonly taken = new Set<u32>();
  /** How deep in `read` this is: a listener runs inside it. */
  private reading = 0;
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
   * edge's ask, as `wake` hears it too. Inside a listener, where the ring is
   * being read under this call, only the guest is rung.
   */
  ring(quota_ms: f64): f64 {
    let wake = this.edge(quota_ms);
    if (this.reading > 0) return wake;
    for (;;) {
      const full = this.cq.len === this.cq.capacity;
      this.read();
      if (!full) return wake;
      wake = sooner(wake, this.edge(0));
    }
  }

  /**
   * A query answered in the ring it is asked in: submitted with ACKREQ
   * behind whatever waits, and rung with no quota until the reply comes back.
   * `take` reads the reply; its packet is valid only inside it.
   */
  call<T>(cmd: u32, pkt: U8Array | null, take: (result: i32, reply: BranchCursor | null) => T): T {
    const tag = this.next_call = (this.next_call + 1) >>> 0;
    let out: { value: T } | null = null;
    this.waiting.set(tag, (result, f) => out = { value: take(result, f) });
    try {
      this.submit(tag, cmd, pkt, FrameFlag.ACKREQ);
      if (this.reading > 0) {
        this.edge(0);
        this.take_ahead(tag);
        if (out === null) throw new Error(`[xpute/doorbell] command 0x${cmd.toString(16)} asked inside a listener found the completion ring full`);
      }
      while (out === null) {
        const pending = this.sq.len;
        this.ring(0);
        if (out === null && pending === 0) throw new Error(`[xpute/doorbell] command 0x${cmd.toString(16)} was never replied to`);
      }
    } finally {
      this.waiting.delete(tag);
    }
    return (out as { value: T }).value;
  }

  private edge(quota_ms: f64): f64 {
    const wake = this.interrupt(quota_ms);
    this.wake?.(wake);
    return wake;
  }

  private read(): void {
    this.reading++;
    try {
      const cq = this.cq;
      for (let at = cq.peek(); at >= 0; at = cq.peek()) {
        const tag = cq.tag(at);
        const cmd = cq.cmd(at);
        const pkt = cq.packet(at);
        const f: BranchCursor | null = pkt ? new TreeReader(pkt).read_branch() : null;
        if (cq.flags(at) & FrameFlag.RES) {
          const waiting = this.waiting.get(tag);
          if (this.taken.delete(tag)) {
            // Read already, by the call that asked, from inside a listener.
          } else if (waiting !== undefined) {
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
      this.reading--;
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

  /** The reply to `tag`, read where it waits in the completion ring. A
   * message not yet consumed holds its slot, so its packet stays whole. */
  private take_ahead(tag: u32): void {
    const cq = this.cq;
    for (let pos = cq.head; pos !== cq.tail; pos = (pos + 1) >>> 0) {
      const at = cq.entry_at(pos);
      if ((cq.flags(at) & FrameFlag.RES) === 0 || cq.tag(at) !== tag) continue;
      const pkt = cq.packet(at);
      this.waiting.get(tag)!(cq.result(at), pkt ? new TreeReader(pkt).read_branch() : null);
      this.taken.add(tag);
      return;
    }
  }
}
