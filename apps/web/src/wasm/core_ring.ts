// @pxl/web/wasm/core_ring.ts

/**
 * The shell's end of the rings (crates/pxl/wasm/src/lib.rs): commands into
 * the submission ring, the kernel rung with `pxl_drain`, and the completion
 * ring read — replies to the calls waiting on them, signals to whoever
 * hears them.
 *
 * Its own piece rather than the builds' because the pools are cut through
 * it (core_pool.ts): the map is laid out before anything that builds a
 * chunk exists, and a pool's kernel is asked for its blocks the same way
 * a chunk is sent its source.
 */

import type { u32 } from "@xpute/core/kit/word.ts";
import { Errno } from "@xpute/core/sys/errno.ts";
import type { U8Array } from "@xpute/core/kit/array.ts";
import { FrameFlag } from "@xpute/core/ipc/frame.ts";
import { Ring } from "@xpute/core/ipc/ring.ts";
import { type BranchCursor, type BranchView, encoder, TreeReader, TreeView } from "@xpute/core/io/marshal/tree/mod.ts";
import type { Cmd } from "@pxl/shared/spec/cmd.ts";

import type { PxlExports } from "@/wasm/pxl.ts";

/** A packet of one branch, its children written by `fill` — what a message
 * carries (@pxl/shared/spec/cmd.ts). */
export function packet(fill: (b: BranchView) => void): U8Array {
  return encoder.encode(new TreeView().branch(fill));
}

/** What the kernel raised: the message's tag, its number, and its packet's
 * branch, valid only inside the call. */
export type SignalListener = (tag: u32, cmd: Cmd, f: BranchCursor | null) => void;

export class CoreRing {
  private readonly sq: Ring;
  private readonly cq: Ring;
  /** The calls waiting on their replies, by tag; and the replies a call
   * made inside a sink read ahead of the completion ring's head, which the
   * read under it then passes over. */
  private next_call: u32 = 0;
  private readonly waiting = new Map<u32, (result: number, f: BranchCursor | null) => void>();
  private readonly taken = new Set<u32>();
  /** How deep in `read` this is: a listener runs inside it. */
  private reading = 0;
  /** Who hears the signals. None before the builds exist, and nothing
   * raises one before there is a chunk. */
  signal: SignalListener | null = null;

  constructor(private readonly pxl: PxlExports) {
    this.sq = new Ring(pxl.memory.buffer, pxl.pxl_ring_sq());
    this.cq = new Ring(pxl.memory.buffer, pxl.pxl_ring_cq());
  }

  /** Makes room in the submission ring: applies what waits, and hands out
   * what that made. */
  pump(): void {
    this.pxl.pxl_drain();
    this.read();
  }

  /** Hands the module the turn: a pump, or inside a listener — where the
   * ring is being read under this call — only the drain, since reading it
   * again here would hand the same message out twice. */
  turn(): void {
    if (this.reading > 0) this.pxl.pxl_drain();
    else this.pump();
  }

  /** Sends a command that asks for no reply. It is applied at the module's
   * next drain, tick or read, in the order it was sent. */
  send(tag: u32, cmd: Cmd, pkt: U8Array | null = null): void {
    this.submit(tag, cmd, pkt, 0);
  }

  /** Applies every command sent so far, now, without handing out what they
   * made: for a caller about to read what it changed, and safe inside a
   * listener, since none runs. */
  settle(): void {
    this.pxl.pxl_drain();
  }

  /**
   * A query answered in the turn it is asked: submitted with ACKREQ behind
   * whatever waits — so a placement just sent is what it sees — and the
   * kernel rung until the reply comes back, the signals before it handed
   * out as `read` always does. `take` reads the reply; its packet is valid
   * only inside it.
   */
  call<T>(cmd: Cmd, pkt: U8Array | null, take: (result: number, reply: BranchCursor | null) => T): T {
    const tag = this.next_call = (this.next_call + 1) >>> 0;
    let out: { value: T } | null = null;
    this.waiting.set(tag, (result, f) => out = { value: take(result, f) });
    try {
      this.submit(tag, cmd, pkt, FrameFlag.ACKREQ);
      if (this.reading > 0) {
        // Inside a listener the ring is being read under this call: the
        // reply is found where it lies, ahead of the head, and left for that
        // read.
        this.pxl.pxl_drain();
        this.take_ahead(tag);
      }
      while (out === null) {
        if (this.reading > 0) throw new Error(`[wasm/core_ring] command 0x${cmd.toString(16)} asked inside a listener found the completion ring full`);
        const posted = this.pxl.pxl_drain();
        this.read();
        if (out === null && posted === 0 && this.sq.len === 0) throw new Error(`[wasm/core_ring] command 0x${cmd.toString(16)} was never replied to`);
      }
    } finally {
      this.waiting.delete(tag);
    }
    return (out as { value: T }).value;
  }

  /** Hands out what the completion ring holds. */
  read(): void {
    this.reading++;
    try {
      this.read_ring();
    } finally {
      this.reading--;
    }
  }

  private submit(tag: u32, cmd: Cmd, pkt: U8Array | null, flags: u32): void {
    for (;;) {
      const errno = this.sq.push(tag, cmd, flags, pkt);
      if (errno === Errno.OK) return;
      if (errno !== Errno.EAGAIN) throw new Error(`[wasm/core_ring] command 0x${cmd.toString(16)} refused by the ring: ${errno}`);
      this.turn();
    }
  }

  private read_ring(): void {
    const cq = this.cq;
    for (let at = cq.peek(); at >= 0; at = cq.peek()) {
      const tag = cq.tag(at);
      const cmd = cq.cmd(at) as Cmd;
      const pkt = cq.packet(at);
      const f: BranchCursor | null = pkt ? new TreeReader(pkt).read_branch() : null;
      if (cq.flags(at) & FrameFlag.RES) {
        const waiting = this.waiting.get(tag);
        if (this.taken.delete(tag)) {
          // Read already, by the call that asked, from inside a listener.
        } else if (waiting !== undefined) {
          waiting(cq.result(at), f);
        } else if (cq.result(at) < 0) {
          console.warn(`[wasm/core_ring] command 0x${cmd.toString(16)} failed: errno ${cq.result(at)}`);
        }
      } else if (this.signal !== null) {
        this.signal(tag, cmd, f);
      } else {
        console.warn(`[wasm/core_ring] signal 0x${cmd.toString(16)} raised with no one to hear it`);
      }
      cq.advance();
    }
  }

  /** The reply to `tag`, read where it waits in the completion ring. A
   * message not yet consumed keeps the arena from rewinding, so its packet
   * stays whole. */
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
