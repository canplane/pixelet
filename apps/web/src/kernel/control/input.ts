// @pxl/web/kernel/control/input.ts

/** The host as an interrupt source: drivers translate their events into these. */

import type { f64, i32, u32 } from "@xpute/kit/abi/word.ts";
import {
  type ActiveTool,
  Command,
  encode_input_hold,
  encode_input_keys,
  encode_input_pointer,
  encode_input_rect,
  encode_input_wheel,
  encode_world_visibility,
  type PointerKind,
  type PointerPhase,
} from "@/kernel/abi/cmd.spec.ts";
import { permissionFollow } from "@platform/sensor.ts";
import { GUEST_DOORBELL } from "@/kernel/mem.ts";
import { request_frame, ring_now, send_input, set_driver, strobe, type Viewport } from "@/kernel/sched.ts";

export function kernel_pointer(phase: PointerPhase, kind: PointerKind, id: i32, button: i32, x: f64, y: f64): void {
  send_input(Command.INPUT_POINTER, encode_input_pointer({ phase, kind, id, button, x, y }));
}

export function kernel_wheel(delta_y: f64): void {
  send_input(Command.INPUT_WHEEL, encode_input_wheel({ delta_y }));
}

export function kernel_keys(x: i32, y: i32): void {
  send_input(Command.INPUT_KEYS, encode_input_keys({ x, y }));
}

export function kernel_rect(left: f64, top: f64, width: f64, height: f64): void {
  GUEST_DOORBELL.send(0, Command.INPUT_RECT, encode_input_rect({ left, top, width, height }));
}

export function kernel_hold(tool: ActiveTool, value: u32, tier: u32): void {
  send_input(Command.INPUT_HOLD, encode_input_hold({ tool, value, tier }));
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
  GUEST_DOORBELL.send(0, Command.WORLD_VISIBILITY, encode_world_visibility({ visible }));
  GUEST_DOORBELL.ring(0);
  if (visible) {
    strobe.start();
    request_frame();
  } else strobe.stop();
}
