// @pxl/web/kernel/control/input.ts

/**
 * The host as an interrupt source (crates/pxl/web/kernel/src/control/camera/
 * input.rs): the pointer, the wheel, where the surface lies, and the world's
 * surface opened, shown and hidden — as a driver hears them, whichever
 * platform's it is (platform/web/surface.tsx, a device's surface). A driver
 * translates its own events into these and decides nothing past that.
 *
 * The kernel keeps no pick of its own, so every pointer event carries what
 * the pointer holds. Visibility is rung at once with no quota: a hidden page
 * gets no animation frame, and hiding is the last moment a phone's tab is
 * sure to get for a pending save.
 */

import type { f64, i32, u32 } from "@xpute/core/abi/word.ts";
import { ActiveTool, Command, encode_input_pointer, encode_input_rect, encode_input_wheel, encode_world_visibility, PointerKind, PointerPhase } from "@/kernel/abi/cmd.spec.ts";
import type { GroundChannelKey } from "@/catalog/cell/cube_type.spec.ts";
import { permissionFollow } from "@platform/sensor.ts";
import { CORE_DOORBELL } from "@/kernel/mem.ts";
import { request_frame, ring_now, send_input, set_driver, strobe, type Viewport } from "@/kernel/sched.ts";

/** What the pointer holds: a tool, the held cube's ground channel (none for
 * a flat color) and type, or the decor kind, by the catalog's numbers, and
 * the object size's tier. The kernel keeps no pick of its own; each pointer
 * event carries this. */
interface Holding {
  tool: ActiveTool;
  channel: GroundChannelKey | null;
  value: u32;
  tier: u32;
}

let holding: Holding = { tool: ActiveTool.Pointer, channel: null, value: 0, tier: 0 };
/** The last hovering pointer, while it is over the surface. */
let last_hover: { kind: PointerKind; id: i32; button: i32; x: f64; y: f64 } | null = null;

function send_pointer(phase: PointerPhase, kind: PointerKind, id: i32, button: i32, x: f64, y: f64): void {
  const { tool, channel, value, tier } = holding;
  send_input(Command.INPUT_POINTER, encode_input_pointer({ phase, kind, id, button, x, y, tool, channel, value, tier }));
}

/** A pointer at (x, y) in the window, in its pixels. A mouse or a pen
 * hovers, and is remembered until it leaves; a touch only presses. */
export function kernel_pointer(phase: PointerPhase, kind: PointerKind, id: i32, button: i32, x: f64, y: f64): void {
  last_hover = phase === PointerPhase.Out || kind === PointerKind.Touch ? null : { kind, id, button, x, y };
  send_pointer(phase, kind, id, button, x, y);
}

/** A wheel's turn over the surface, in CSS pixels. */
export function kernel_wheel(delta_y: f64): void {
  send_input(Command.INPUT_WHEEL, encode_input_wheel({ delta_y }));
}

/** Where the surface lies in the window, in its pixels: what a pointer's
 * place is read against. */
export function kernel_rect(left: f64, top: f64, width: f64, height: f64): void {
  CORE_DOORBELL.send(0, Command.INPUT_RECT, encode_input_rect({ left, top, width, height }));
}

/**
 * Sets what the pointer holds. A hover under a still pointer would otherwise
 * show the old pick until the pointer moves, so a move at the last position
 * is sent, as a browser synthesizes a mouse move when what is under a still
 * cursor changes.
 */
export function kernel_hold(tool: ActiveTool, channel: GroundChannelKey | null, value: u32, tier: u32): void {
  holding = { tool, channel, value, tier };
  if (last_hover) send_pointer(PointerPhase.Move, last_hover.kind, last_hover.id, last_hover.button, last_hover.x, last_hover.y);
}

/** The world's surface is up, sized as `viewport` reads it every frame: the
 * turns run, the sensors open, and a permission granted from outside the app
 * is followed. What it returns closes all of it. */
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

/** Whether the app is on screen. Rung at once with no quota, since a hidden
 * app gets no frame and hiding is its last sure moment for the pending save;
 * the turns stop while it is hidden and resume when it is back. */
export function kernel_visible(visible: boolean): void {
  CORE_DOORBELL.send(0, Command.WORLD_VISIBILITY, encode_world_visibility({ visible }));
  CORE_DOORBELL.ring(0);
  if (visible) {
    strobe.start();
    request_frame();
  } else strobe.stop();
}
