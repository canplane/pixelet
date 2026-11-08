// @pxl/client/service/core/input.ts

import type { f64, i32 } from "@xpute/kit/abi/word.ts";
import { PointerPhase, PointerType } from "@/abi/cmd.spec.ts";
import {
  type PointerKind,
  type PointerPhase as PlacePhase,
} from "@/service/platform/types.ts";
import * as sensor from "@/service/platform/sensor.ts";
import { core, ui } from "@/service/core/core.ts";

/** Asks that the next turn run as an interactive one, as any input does. */
export function interactive(): void {
  ui.input();
}

export function pointer(
  phase: PointerPhase,
  pointer_type: PointerType,
  id: i32,
  button: i32,
  x: f64,
  y: f64,
): void {
  interactive();
  core.input_pointer(phase, pointer_type, id, button, x, y);
}

export function wheel(delta_y: f64): void {
  interactive();
  core.input_wheel(delta_y);
}

export function keys(x: i32, y: i32): void {
  interactive();
  core.input_move(x, y);
}

/** Opens the world's view and returns the function that closes it. Turns run,
 * and the sensors are followed, only while the view is open. */
export function open(): () => void {
  ui.resume();
  const unfollow = sensor.follow();
  return () => {
    unfollow();
    ui.pause();
  };
}

/** Asks for a turn now, as a box that moved needs one. */
export function frame(): void {
  ui.wake();
}

export const POINTER_PHASE: Readonly<Record<PlacePhase, PointerPhase>> = {
  down: PointerPhase.Down,
  move: PointerPhase.Move,
  up: PointerPhase.Up,
  cancel: PointerPhase.Cancel,
  out: PointerPhase.Out,
};

export const POINTER_KIND: Readonly<Record<PointerKind, PointerType>> = {
  mouse: PointerType.Mouse,
  touch: PointerType.Touch,
  pen: PointerType.Pen,
};
