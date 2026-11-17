// @pxl/client/service/core/input.ts

import type { f64, i32 } from "@xpute/kit/abi/word.ts";
import {
  type Drag,
  type PointerPhase,
  DRAG_NONE,
  DRAG_PAN,
  DRAG_ROTATE,
  POINTER_PHASE_DOWN,
  POINTER_PHASE_MOVE,
  POINTER_PHASE_UP,
  POINTER_PHASE_CANCEL,
  POINTER_PHASE_OUT,
} from "@/abi/input.enum.ts";
import {
  type PointerKind,
  type PointerPhase as PlacePhase,
} from "@/service/platform/types.ts";
import * as input from "@/abi/input.ts";
import { ui } from "@/service/core/core.ts";

export function pointer(
  phase: PointerPhase,
  touch: boolean,
  id: i32,
  drag: Drag,
  x: f64,
  y: f64,
): void {
  void input.pointer(ui, phase, touch, id, drag, x, y);
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

export const POINTER_PHASE: Readonly<Record<PlacePhase, PointerPhase>> = {
  down: POINTER_PHASE_DOWN,
  move: POINTER_PHASE_MOVE,
  up: POINTER_PHASE_UP,
  cancel: POINTER_PHASE_CANCEL,
  out: POINTER_PHASE_OUT,
};

/** What a press drags by its button, as the web numbers buttons: the primary
 * pans, the secondary turns, and any other drags nothing. */
export function drag_of(button: i32): Drag {
  return button === 0 ? DRAG_PAN : button === 2 ? DRAG_ROTATE : DRAG_NONE;
}

/** Whether a pointer of each kind is a finger on a screen (pxl:core input
 * `pointer`'s `touch`). */
export const TOUCH: Readonly<Record<PointerKind, boolean>> = {
  mouse: false,
  touch: true,
  pen: false,
};
