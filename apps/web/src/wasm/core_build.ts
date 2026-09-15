// @pxl/web/wasm/core_build.ts

/**
 * The shell's channel to the kernel over the rings (core_ring.ts): commands
 * sent, calls answered, the central scheduler's strobe, and the signals a
 * turn or an action leaves for the shell handed to whoever hears them
 * (wasm/kernel.ts). The kernel builds, schedules and draws its chunks inside;
 * none of that crosses here.
 */

import type { u32 } from "@xpute/core/kit/word.ts";
import type { U8Array } from "@xpute/core/kit/array.ts";
import type { BranchCursor } from "@xpute/core/io/marshal/tree/mod.ts";
import type { Cmd } from "@pxl/shared/spec/cmd.ts";

import { CORE_RING } from "@/wasm/core_memory.ts";
import { type CoreRing, packet, type SignalListener } from "@/wasm/core_ring.ts";

export { packet };

export class CoreBuild {
  /** Who hears what a turn or an action asks of the shell. */
  shell: SignalListener | null = null;

  constructor(private readonly ring: CoreRing) {
    ring.signal = (tag, cmd, f) => {
      if (this.shell !== null) this.shell(tag, cmd, f);
      else console.warn(`[wasm/core_build] signal 0x${cmd.toString(16)} raised with no shell to hear it`);
    };
  }

  /** Makes room in the submission ring: applies what waits, and hands out
   * what that made. */
  pump(): void {
    this.ring.pump();
  }

  /** The ring's send (core_ring.ts). */
  send(tag: u32, cmd: Cmd, pkt: U8Array | null = null): void {
    this.ring.send(tag, cmd, pkt);
  }

  /** The ring's settle (core_ring.ts). */
  settle(): void {
    this.ring.settle();
  }

  /** The central scheduler's strobe: `cmd` submitted, the kernel handed the
   * turn, and what the turn left on the completion ring read until it is
   * dry. */
  strobe(cmd: Cmd, pkt: U8Array): void {
    this.ring.send(0, cmd, pkt);
    this.ring.pump();
    while (this.ring.drain() > 0) this.ring.read();
  }

  /** The ring's call (core_ring.ts). */
  call<T>(cmd: Cmd, pkt: U8Array | null, take: (result: number, reply: BranchCursor | null) => T): T {
    return this.ring.call(cmd, pkt, take);
  }
}

/** The one channel to the kernel. */
export const CORE_BUILD: CoreBuild = new CoreBuild(CORE_RING);
