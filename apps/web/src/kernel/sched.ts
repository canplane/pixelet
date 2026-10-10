// @pxl/web/kernel/sched.ts

/**
 * The central scheduler's turns for the kernel: a frame submits its viewport,
 * gap and I/O credits, then rings with a quota.
 */

import type { U8Array } from "@xpute/core/abi/array.ts";
import type { f64, u32 } from "@xpute/core/abi/word.ts";
import { Strobe } from "@xpute/host/sched/strobe.ts";
import { Command, encode_world_frame } from "@/kernel/abi/cmd.spec.ts";
import { CORE, CORE_DOORBELL } from "@/kernel/mem.ts";
import { directory } from "@/kernel/module.ts";

export interface Viewport {
  width: f64;
  height: f64;
  dpr: f64;
}

/** Asked every frame, so a resize needs no event of its own. */
let driver: (() => Viewport) | null = null;

export function request_frame(): void {
  strobe.request();
}

export function send_next(cmd: u32, pkt?: U8Array): void {
  CORE_DOORBELL.send(0, cmd, pkt);
  request_frame();
}

export function send_input(cmd: u32, pkt?: U8Array): void {
  CORE_DOORBELL.send(0, cmd, pkt);
  strobe.input();
  request_frame();
}

/** Rung at once with no quota, so it is heard even after the strobe has stopped. */
export function ring_now(cmd: Command, pkt?: U8Array): void {
  CORE_DOORBELL.send(0, cmd, pkt);
  CORE_DOORBELL.ring(0);
  request_frame();
}

export function set_driver(viewport: (() => Viewport) | null): void {
  driver = viewport;
}

/** A browser's ~6 connections per origin, so the kernel's queue, not the socket pool, orders the work. */
const NET_CREDITS = 6;

export let strobe: Strobe;

export function start_sched(): void {
  strobe = new Strobe(CORE_DOORBELL, directory(CORE.buffer).quantum(), (grant, delta_s) => {
    if (driver === null) return;
    const { width: w, height: h, dpr } = driver();
    const width = Math.max(1, w);
    const height = Math.max(1, h);
    const unix_ms = Date.now();
    CORE_DOORBELL.send(
      0,
      Command.WORLD_FRAME,
      encode_world_frame({
        width,
        height,
        dpr,
        delta_s,
        frame_ms: grant.frame_ms,
        interactive: grant.interactive,
        credits: NET_CREDITS,
        unix_ms,
      }),
    );
  }, (run) => requestAnimationFrame(run));
}
