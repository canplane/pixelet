// @pxl/client/abi/control/input.ts

/** Input as the kernel's commands, which drivers translate their events
 * into, and what each frame the strobe grants submits with the viewport. */

import type { f64, i32, u32 } from "@xpute/kit/abi/word.ts";
import type { Grant } from "@xpute/host/sched/quantum.ts";
import {
  type ActiveTool,
  Command,
  encode_input_hold,
  encode_input_keys,
  encode_input_pointer,
  encode_input_rect,
  encode_input_wheel,
  encode_world_frame,
  encode_world_visibility,
  type PointerPhase,
  type PointerType,
} from "@/abi/cmd.spec.ts";
import { permissionFollow } from "@/bracket.tsx";
import { GUEST_DOORBELL } from "@/host/mem.ts";
import { request_frame, ring_now, send_input, strobe } from "@/host/sched.ts";

export interface Viewport {
  width: f64;
  height: f64;
  dpr: f64;
}

/** Asked every frame, so a resize needs no event of its own. */
let driver: (() => Viewport) | null = null;

/** A browser's ~6 connections per origin, so the kernel's queue, not the socket pool, orders the work. */
const NET_CREDITS = 6;

/** What each frame the strobe grants submits: the viewport, the display's
 * pacing, whether it is interactive, the reads allowed out and the clock. */
export function submit_frame(grant: Grant): void {
  if (driver === null) return;
  const { width: w, height: h } = driver();
  const width = Math.max(1, w);
  const height = Math.max(1, h);
  GUEST_DOORBELL.send(0, Command.WORLD_FRAME, encode_world_frame(GUEST_DOORBELL, width, height, grant.refresh_ms, grant.paced_ms, grant.interactive, NET_CREDITS, Date.now()));
}

export function kernel_pointer(phase: PointerPhase, pointer_type: PointerType, id: i32, button: i32, x: f64, y: f64): void {
  send_input(Command.INPUT_POINTER, encode_input_pointer(GUEST_DOORBELL, phase, pointer_type, id, button, x, y));
}

export function kernel_wheel(delta_y: f64): void {
  send_input(Command.INPUT_WHEEL, encode_input_wheel(GUEST_DOORBELL, delta_y));
}

export function kernel_keys(x: i32, y: i32): void {
  send_input(Command.INPUT_KEYS, encode_input_keys(GUEST_DOORBELL, x, y));
}

export function kernel_rect(left: f64, top: f64, width: f64, height: f64): void {
  GUEST_DOORBELL.send(0, Command.INPUT_RECT, encode_input_rect(GUEST_DOORBELL, left, top, width, height));
}

export function kernel_hold(tool: ActiveTool, value: u32, tier: u32): void {
  send_input(Command.INPUT_HOLD, encode_input_hold(GUEST_DOORBELL, tool, value, tier));
}

export function kernel_open(viewport: () => Viewport): () => void {
  driver = viewport;
  ring_now(Command.SENSOR_OPEN);
  strobe.start();
  const unfollow = permissionFollow();
  return () => {
    unfollow();
    strobe.stop();
    ring_now(Command.SENSOR_CLOSE);
    driver = null;
  };
}

/** Rung at once with no quota: a hidden page gets no frame, and this is the last sure moment for a save. */
export function kernel_visible(visible: boolean): void {
  GUEST_DOORBELL.send(0, Command.WORLD_VISIBILITY, encode_world_visibility(GUEST_DOORBELL, visible));
  GUEST_DOORBELL.ring(0);
  if (visible) {
    strobe.start();
    request_frame();
  } else strobe.stop();
}
