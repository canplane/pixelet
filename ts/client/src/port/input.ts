// @pxl/client/port/input.ts

/** Input as the kernel's commands, from the place's words (`POINTER_PHASE`,
 * `POINTER_KIND`), and the world's view opened and closed. */

import type { f64, i32, u32 } from "@xpute/kit/abi/word.ts";
import type { PacketWriter } from "@xpute/kit/wire/xtp/mod.ts";
import {
  type ActiveTool,
  Command,
  encode_input_hold,
  encode_input_keys,
  encode_input_pointer,
  encode_input_rect,
  encode_input_wheel,
  encode_world_view,
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

export function kernel_pointer(
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

export function kernel_wheel(delta_y: f64): void {
  send_input(Command.INPUT_WHEEL, encode_input_wheel(HOST.doorbell, delta_y));
}

export function kernel_keys(x: i32, y: i32): void {
  send_input(Command.INPUT_KEYS, encode_input_keys(HOST.doorbell, x, y));
}

export function kernel_rect(
  left: f64,
  top: f64,
  width: f64,
  height: f64,
): void {
  HOST.doorbell.send(
    port.SHELL,
    Command.INPUT_RECT,
    encode_input_rect(HOST.doorbell, left, top, width, height),
  );
}

export function kernel_hold(tool: ActiveTool, val: u32, tier: u32): void {
  send_input(
    Command.INPUT_HOLD,
    encode_input_hold(HOST.doorbell, tool, val, tier),
  );
}

export function kernel_open(): () => void {
  HOST.now(
    port.SHELL,
    Command.WORLD_VIEW,
    encode_world_view(HOST.doorbell, true),
  );
  set_view_open(true);
  const unfollow = bracket.sensor.follow();
  return () => {
    unfollow();
    set_view_open(false);
    HOST.now(
      port.SHELL,
      Command.WORLD_VIEW,
      encode_world_view(HOST.doorbell, false),
    );
  };
}

/** A turn asked for now, as a moved box needs one. */
export function kernel_frame(): void {
  HOST.strobe.request();
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
