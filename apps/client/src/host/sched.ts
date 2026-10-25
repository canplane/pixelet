// @pxl/client/host/sched.ts

/**
 * The central scheduler's turns for the guest: each granted frame is
 * submitted by what the boot hands in, then rung with a quota.
 */

import type { u32 } from "@xpute/kit/abi/word.ts";
import type { Grant } from "@xpute/host/sched/quantum.ts";
import { Strobe } from "@xpute/host/sched/strobe.ts";
import type { PacketWriter } from "@xpute/kit/wire/xtp/mod.ts";
import { GUEST_DIRECTORY, GUEST_DOORBELL } from "@/host/mem.ts";

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
export function ring_now(cmd: u32, pkt?: PacketWriter): void {
  GUEST_DOORBELL.send(0, cmd, pkt);
  GUEST_DOORBELL.ring(0);
  request_frame();
}

export let strobe: Strobe;

/** The quantum is the guest's, from its directory; `submit` fills the rings
 * before each frame it grants is rung. */
export function start_sched(submit: (grant: Grant) => void): void {
  strobe = new Strobe(GUEST_DOORBELL, GUEST_DIRECTORY.quantum(), submit, (run) => requestAnimationFrame(run));
}
