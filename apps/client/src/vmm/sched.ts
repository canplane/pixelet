// @pxl/client/vmm/sched.ts

/**
 * The central scheduler's turns for the kernel: a frame submits its viewport,
 * gap and I/O credits, then rings with a quota.
 */

import type { f64, u32 } from "@xpute/kit/abi/word.ts";
import { Strobe } from "@xpute/hypervisor/sched/strobe.ts";
import type { PacketWriter } from "@xpute/kit/wire/xtp/mod.ts";
import { Command, encode_world_frame } from "@/vmm/abi/cmd.spec.ts";
import { GUEST, GUEST_DOORBELL } from "@/vmm/mem.ts";
import { directory } from "@/vmm/module.ts";

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

export function send_next(cmd: u32, pkt?: PacketWriter): void {
  GUEST_DOORBELL.send(0, cmd, pkt);
  request_frame();
}

export function send_input(cmd: u32, pkt?: PacketWriter): void {
  GUEST_DOORBELL.send(0, cmd, pkt);
  strobe.input();
  request_frame();
}

/** Rung at once with no quota, so it is heard even after the strobe has stopped. */
export function ring_now(cmd: Command, pkt?: PacketWriter): void {
  GUEST_DOORBELL.send(0, cmd, pkt);
  GUEST_DOORBELL.ring(0);
  request_frame();
}

export function set_driver(viewport: (() => Viewport) | null): void {
  driver = viewport;
}

/** A browser's ~6 connections per origin, so the kernel's queue, not the socket pool, orders the work. */
const NET_CREDITS = 6;

export let strobe: Strobe;

export function start_sched(): void {
  strobe = new Strobe(GUEST_DOORBELL, directory(GUEST.buffer).quantum(), (grant) => {
    if (driver === null) return;
    const { width: w, height: h } = driver();
    const width = Math.max(1, w);
    const height = Math.max(1, h);
    GUEST_DOORBELL.send(0, Command.WORLD_FRAME, encode_world_frame(GUEST_DOORBELL, width, height, grant.refresh_ms, grant.paced_ms, grant.interactive, NET_CREDITS, Date.now()));
  }, (run) => requestAnimationFrame(run));
}
