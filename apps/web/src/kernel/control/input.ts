// @pxl/web/kernel/control/input.ts

/**
 * The host as an interrupt source: drivers translate their events into these.
 * The kernel keeps no pick, so every pointer event carries what the pointer
 * holds.
 */

import type { f64, i32, u32 } from "@xpute/core/abi/word.ts";
import { ActiveTool, Command, encode_input_pointer, encode_input_rect, encode_input_wheel, encode_world_visibility, PointerKind, PointerPhase } from "@/kernel/abi/cmd.spec.ts";
import type { GroundChannelKey } from "@/catalog/cell/cube_type.spec.ts";
import { permissionFollow } from "@platform/sensor.ts";
import { CORE_DOORBELL } from "@/kernel/mem.ts";
import { request_frame, ring_now, set_driver, strobe, type Viewport } from "@/kernel/sched.ts";

interface Holding {
  tool: ActiveTool;
  channel: GroundChannelKey | null;
  value: u32;
  tier: u32;
}

let holding: Holding = { tool: ActiveTool.Pointer, channel: null, value: 0, tier: 0 };
let last_hover: { kind: PointerKind; id: i32; button: i32; x: f64; y: f64 } | null = null;

function send_pointer(phase: PointerPhase, kind: PointerKind, id: i32, button: i32, x: f64, y: f64): void {
  const { tool, channel, value, tier } = holding;
  CORE_DOORBELL.send(0, Command.INPUT_POINTER, encode_input_pointer({ phase, kind, id, button, x, y, tool, channel, value, tier }));
  strobe.input();
  request_frame();
}

export function kernel_pointer(phase: PointerPhase, kind: PointerKind, id: i32, button: i32, x: f64, y: f64): void {
  last_hover = phase === PointerPhase.Out || kind === PointerKind.Touch ? null : { kind, id, button, x, y };
  send_pointer(phase, kind, id, button, x, y);
}

export function kernel_wheel(delta_y: f64): void {
  CORE_DOORBELL.send(0, Command.INPUT_WHEEL, encode_input_wheel({ delta_y }));
  strobe.input();
  request_frame();
}

export function kernel_rect(left: f64, top: f64, width: f64, height: f64): void {
  CORE_DOORBELL.send(0, Command.INPUT_RECT, encode_input_rect({ left, top, width, height }));
}

/** A move at the last position is sent, so a hover under a still pointer shows the new pick. */
export function kernel_hold(tool: ActiveTool, channel: GroundChannelKey | null, value: u32, tier: u32): void {
  holding = { tool, channel, value, tier };
  if (last_hover) send_pointer(PointerPhase.Move, last_hover.kind, last_hover.id, last_hover.button, last_hover.x, last_hover.y);
}

export function kernel_open(viewport: () => Viewport): () => void {
  set_driver(viewport);
  ring_now(Command.SENSOR_OPEN);
  strobe.start();
  const unfollow = permissionFollow();
  return () => {
    unfollow();
    strobe.stop();
    ring_now(Command.SENSOR_CLOSE);
    set_driver(null);
  };
}

/** Rung at once with no quota: a hidden page gets no frame, and this is the last sure moment for a save. */
export function kernel_visible(visible: boolean): void {
  CORE_DOORBELL.send(0, Command.WORLD_VISIBILITY, encode_world_visibility({ visible }));
  CORE_DOORBELL.ring(0);
  if (visible) {
    strobe.start();
    request_frame();
  } else strobe.stop();
}
