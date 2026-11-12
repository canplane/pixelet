// @pxl/client/service/core/input.ts

import type { f64, i32 } from "@xpute/kit/abi/word.ts";
import { PointerPhase, PointerType } from "@/abi/input.ts";
import {
  type PointerKind,
  type PointerPhase as PlacePhase,
} from "@/service/platform/types.ts";
import * as sensor from "@/service/platform/sensor.ts";
import * as input from "@/abi/input.ts";
import { core } from "@/service/core/core.ts";

export function pointer(
  phase: PointerPhase,
  pointer_type: PointerType,
  id: i32,
  button: i32,
  x: f64,
  y: f64,
): void {
  void input.pointer(core.input, phase, pointer_type, id, button, x, y);
}

export function typing(on: boolean): void {
  void input.typing(core.input, on);
}

export function wheel(delta_y: f64): void {
  void input.wheel(core.input, delta_y);
}

export function pinch(scale: f64): void {
  void input.pinch(core.input, scale);
}

export function keys(x: i32, y: i32): void {
  void input.move(core.input, x, y);
}

/** Follows the sensors while the world's view is open: called as it opens,
 * it returns the function called as it closes. */
export function open(): () => void {
  return sensor.follow();
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
