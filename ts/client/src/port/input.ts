// @pxl/client/port/input.ts

/** Input as the core's commands, from the place's words (`POINTER_PHASE`,
 * `POINTER_KIND`), and the world's view opened and closed. */

import type { f64, i32 } from "@xpute/kit/abi/word.ts";
import { PointerPhase, PointerType } from "@/abi/cmd.spec.ts";
import {
  type PointerKind,
  type PointerPhase as PlacePhase,
} from "@/service/types.ts";
import * as sensor from "@/service/sensor.ts";
import { core, ui } from "@/port/core.ts";

/** The next turn made interactive, as an input asks. */
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

/** The world's view opened: turns run while it is open. */
export function open(): () => void {
  ui.resume();
  const unfollow = sensor.follow();
  return () => {
    unfollow();
    ui.pause();
  };
}

/** A turn asked for now, as a moved box needs one. */
export function frame(): void {
  ui.wake();
}

/** A place's pointer phase, as the core numbers it. */
export const POINTER_PHASE: Readonly<Record<PlacePhase, PointerPhase>> = {
  down: PointerPhase.Down,
  move: PointerPhase.Move,
  up: PointerPhase.Up,
  cancel: PointerPhase.Cancel,
  out: PointerPhase.Out,
};

/** A place's pointer kind, as the core numbers it. */
export const POINTER_KIND: Readonly<Record<PointerKind, PointerType>> = {
  mouse: PointerType.Mouse,
  touch: PointerType.Touch,
  pen: PointerType.Pen,
};
