// @pxl/client/abi/input.ts

/** Input as the kernel's commands, from the place's words (`POINTER_PHASE`,
 * `POINTER_KIND`), and what each frame the strobe grants submits with the viewport. */

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
  encode_world_view,
  encode_world_visibility,
  PointerPhase,
  PointerType,
} from "@/abi/cmd.spec.ts";
import { bracket, type PointerKind, type PointerPhase as PlacePhase, type Viewport } from "@/bracket.tsx";
import { HOST } from "@/host/mount.ts";

/** Asked every frame, so a resize needs no event of its own. */
let driver: (() => Viewport) | null = null;

/** What each frame the strobe grants submits: the viewport, the display's
 * pacing, whether it is interactive, the reads allowed out and the clock. */
export function submit_frame(grant: Grant): void {
  if (driver === null) return;
  const { width: w, height: h } = driver();
  const width = Math.max(1, w);
  const height = Math.max(1, h);
  HOST.doorbell.send(0, Command.WORLD_FRAME, encode_world_frame(HOST.doorbell, width, height, grant.refresh_ms, grant.paced_ms, grant.interactive, bracket.origin.credits, Date.now()));
}

export function kernel_pointer(phase: PointerPhase, pointer_type: PointerType, id: i32, button: i32, x: f64, y: f64): void {
  HOST.input(Command.INPUT_POINTER, encode_input_pointer(HOST.doorbell, phase, pointer_type, id, button, x, y));
}

export function kernel_wheel(delta_y: f64): void {
  HOST.input(Command.INPUT_WHEEL, encode_input_wheel(HOST.doorbell, delta_y));
}

export function kernel_keys(x: i32, y: i32): void {
  HOST.input(Command.INPUT_KEYS, encode_input_keys(HOST.doorbell, x, y));
}

export function kernel_rect(left: f64, top: f64, width: f64, height: f64): void {
  HOST.doorbell.send(0, Command.INPUT_RECT, encode_input_rect(HOST.doorbell, left, top, width, height));
}

export function kernel_hold(tool: ActiveTool, value: u32, tier: u32): void {
  HOST.input(Command.INPUT_HOLD, encode_input_hold(HOST.doorbell, tool, value, tier));
}

export function kernel_open(viewport: () => Viewport): () => void {
  driver = viewport;
  HOST.now(Command.WORLD_VIEW, encode_world_view(HOST.doorbell, true));
  HOST.strobe.start();
  const unfollow = bracket.sensor.follow();
  return () => {
    unfollow();
    HOST.strobe.stop();
    HOST.now(Command.WORLD_VIEW, encode_world_view(HOST.doorbell, false));
    driver = null;
  };
}

/** Rung at once with no quota: a hidden page gets no frame, and this is the last sure moment for a save. */
export function kernel_visible(visible: boolean): void {
  HOST.doorbell.send(0, Command.WORLD_VISIBILITY, encode_world_visibility(HOST.doorbell, visible));
  HOST.doorbell.ring(0);
  if (visible) {
    HOST.strobe.start();
    HOST.strobe.request();
  } else HOST.strobe.stop();
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
