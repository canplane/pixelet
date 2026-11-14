// @pxl/client/service/core/input.ts

import type { f64, i32 } from "@xpute/kit/abi/word.ts";
import {
  type PointerPhase,
  type PointerType,
  POINTER_PHASE_DOWN,
  POINTER_PHASE_MOVE,
  POINTER_PHASE_UP,
  POINTER_PHASE_CANCEL,
  POINTER_PHASE_OUT,
  POINTER_TYPE_MOUSE,
  POINTER_TYPE_TOUCH,
  POINTER_TYPE_PEN,
} from "@/abi/input.enum.ts";
import {
  type PointerKind,
  type PointerPhase as PlacePhase,
} from "@/service/platform/types.ts";
import * as sensor from "@/service/platform/sensor.ts";
import * as input from "@/abi/input.ts";
import { ui } from "@/service/core/core.ts";

export function pointer(
  phase: PointerPhase,
  pointer_type: PointerType,
  id: i32,
  button: i32,
  x: f64,
  y: f64,
): void {
  void input.pointer(ui, phase, pointer_type, id, button, x, y);
}

export function typing(on: boolean): void {
  void input.typing(ui, on);
}

export function wheel(delta_y: f64): void {
  void input.wheel(ui, delta_y);
}

export function pinch(scale: f64): void {
  void input.pinch(ui, scale);
}

export function move(x: i32, y: i32): void {
  void input.move(ui, x, y);
}

/** Follows the sensors while the world's view is open: called as it opens,
 * it returns the function called as it closes. */
export function open(): () => void {
  return sensor.follow();
}

export const POINTER_PHASE: Readonly<Record<PlacePhase, PointerPhase>> = {
  down: POINTER_PHASE_DOWN,
  move: POINTER_PHASE_MOVE,
  up: POINTER_PHASE_UP,
  cancel: POINTER_PHASE_CANCEL,
  out: POINTER_PHASE_OUT,
};

export const POINTER_KIND: Readonly<Record<PointerKind, PointerType>> = {
  mouse: POINTER_TYPE_MOUSE,
  touch: POINTER_TYPE_TOUCH,
  pen: POINTER_TYPE_PEN,
};
