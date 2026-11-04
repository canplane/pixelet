// @pxl/client/port/input.ts

/** Input as the kernel's commands, from the place's words (`POINTER_PHASE`,
 * `POINTER_KIND`), and the world's view opened and closed. */

import type { f64, i32 } from "@xpute/kit/abi/word.ts";
import type { PacketWriter } from "@xpute/kit/wire/xtp/mod.ts";
import {
  Command,
  encode_input_move,
  encode_input_pointer,
  encode_input_wheel,
  PointerPhase,
  PointerType,
} from "@/abi/cmd.spec.ts";
import {
  bracket,
  type PointerKind,
  type PointerPhase as PlacePhase,
} from "@/bracket.tsx";
import { HOST, PACING, set_view_open } from "@/host/mount.ts";
import * as port from "@/abi/port.spec.ts";

/** Sent with the next turn, which it makes interactive. */
export function send_input(cmd: Command, pkt: PacketWriter): void {
  PACING.input(performance.now());
  HOST.send(port.SHELL, cmd, pkt);
}

export function pointer(
  phase: PointerPhase,
  pointer_type: PointerType,
  id: i32,
  button: i32,
  x: f64,
  y: f64,
): void {
  send_input(
    Command.INPUT_POINTER,
    encode_input_pointer(HOST.doorbell, phase, pointer_type, id, button, x, y),
  );
}

export function wheel(delta_y: f64): void {
  send_input(Command.INPUT_WHEEL, encode_input_wheel(HOST.doorbell, delta_y));
}

export function keys(x: i32, y: i32): void {
  send_input(Command.INPUT_MOVE, encode_input_move(HOST.doorbell, x, y));
}

export function open(): () => void {
  set_view_open(true);
  const unfollow = bracket.sensor.follow();
  return () => {
    unfollow();
    set_view_open(false);
  };
}

/** A turn asked for now, as a moved box needs one. */
export function frame(): void {
  HOST.wake();
}

/** A place's pointer phase, as the kernel numbers it. */
export const POINTER_PHASE: Readonly<Record<PlacePhase, PointerPhase>> = {
  down: PointerPhase.Down,
  move: PointerPhase.Move,
  up: PointerPhase.Up,
  cancel: PointerPhase.Cancel,
  out: PointerPhase.Out,
};

/** A place's pointer kind, as the kernel numbers it. */
export const POINTER_KIND: Readonly<Record<PointerKind, PointerType>> = {
  mouse: PointerType.Mouse,
  touch: PointerType.Touch,
  pen: PointerType.Pen,
};
