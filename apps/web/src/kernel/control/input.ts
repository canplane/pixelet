// @pxl/web/kernel/control/input.ts

/**
 * The page as an interrupt source
 * (crates/pxl/web/kernel/src/control/camera/input.rs): the pointer's events
 * over the canvas, the wheel, the tab's visibility, and the one frame loop
 * that mounts them.
 *
 * The kernel keeps no pick of its own, so every pointer event carries what
 * the pointer holds. Visibility is rung at once with no quota: a hidden page
 * gets no animation frame, and hiding is the last moment a phone's tab is
 * sure to get for a pending save.
 */

import { useLayoutEffect } from "react";

import type { f64, i32, u32 } from "@xpute/core/abi/word.ts";
import {
  ActiveTool,
  Command,
  encode_input_pointer,
  encode_input_update,
  encode_input_wheel,
  encode_sensor_visibility,
  PermissionAnswer,
  PointerKind,
  PointerPhase,
  Sensor,
} from "@/kernel/abi/cmd.spec.ts";
import type { GroundChannelKey } from "@/catalog/cell/cube_type.spec.ts";
import { send_permission } from "@/kernel/control/sensor.ts";
import { CORE_BUILD } from "@/kernel/ipc.ts";
import { request_frame, set_driver, strobe } from "@/kernel/sched.ts";
import { kernel_command } from "@/kernel/shell.ts";

export function send_rect(element: HTMLElement): void {
  const r = element.getBoundingClientRect();
  CORE_BUILD.send(0, Command.INPUT_UPDATE, encode_input_update({ left: r.left, top: r.top, width: r.width, height: r.height }));
}

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
/** The last hovering pointer, while it is over the canvas. */
let last_hover: { kind: PointerKind; id: i32; button: i32; x: f64; y: f64 } | null = null;

function send_pointer_at(phase: PointerPhase, kind: PointerKind, id: i32, button: i32, x: f64, y: f64): void {
  const { tool, channel, value, tier } = holding;
  CORE_BUILD.send(0, Command.INPUT_POINTER, encode_input_pointer({ phase, kind, id, button, x, y, tool, channel, value, tier }));
  strobe.input();
  request_frame();
}

function send_pointer(phase: PointerPhase, e: PointerEvent): void {
  const kind = e.pointerType === "touch" ? PointerKind.Touch : e.pointerType === "pen" ? PointerKind.Pen : PointerKind.Mouse;
  last_hover = phase === PointerPhase.Out || kind === PointerKind.Touch ? null : { kind, id: e.pointerId, button: e.button, x: e.clientX, y: e.clientY };
  send_pointer_at(phase, kind, e.pointerId, e.button, e.clientX, e.clientY);
}

/**
 * Sets what the pointer holds. A hover under a still pointer would otherwise
 * show the old pick until the pointer moves, so a move at the last position
 * is sent, as a browser synthesizes a mouse move when what is under a still
 * cursor changes.
 */
export function kernel_hold(tool: ActiveTool, channel: GroundChannelKey | null, value: u32, tier: u32): void {
  holding = { tool, channel, value, tier };
  if (last_hover) send_pointer_at(PointerPhase.Move, last_hover.kind, last_hover.id, last_hover.button, last_hover.x, last_hover.y);
}

/**
 * The one frame loop over `element`: the kernel's turns, the pointer's events
 * on it, the tab's visibility and the sensors' open and close.
 */
export function KernelDriver({ element }: { element: HTMLElement | null }): null {
  useLayoutEffect(() => {
    if (!element) return;
    set_driver(element);

    const on_down = (e: PointerEvent) => {
      send_rect(element);
      send_pointer(PointerPhase.Down, e);
    };
    const on_move = (e: PointerEvent) => send_pointer(PointerPhase.Move, e);
    const on_up = (e: PointerEvent) => send_pointer(PointerPhase.Up, e);
    const on_leave = (e: PointerEvent) => send_pointer(PointerPhase.Out, e);
    const on_wheel = (e: WheelEvent) => {
      if (e.ctrlKey) e.preventDefault();
      CORE_BUILD.send(0, Command.INPUT_WHEEL, encode_input_wheel({ delta_y: e.deltaY }));
      strobe.input();
      request_frame();
    };
    const on_context_menu = (e: MouseEvent) => e.preventDefault();
    const on_resize = () => {
      send_rect(element);
      request_frame();
    };
    // Rung at once with no quota: a hidden page gets no animation frame, and
    // hiding is the last moment a phone's tab is sure to get for the pending
    // save.
    const send_visibility = (visible: boolean) => {
      CORE_BUILD.send(0, Command.SENSOR_VISIBILITY, encode_sensor_visibility({ visible }));
      CORE_BUILD.ring(0);
    };
    const on_visibility = () => {
      send_visibility(document.visibilityState === "visible");
      request_frame();
    };
    const on_page_hide = () => send_visibility(false);

    element.addEventListener("pointerdown", on_down);
    element.addEventListener("pointermove", on_move);
    element.addEventListener("pointerleave", on_leave);
    element.addEventListener("contextmenu", on_context_menu);
    element.addEventListener("wheel", on_wheel, { passive: false });
    globalThis.addEventListener("resize", on_resize);
    globalThis.addEventListener("scroll", on_resize, true);
    globalThis.addEventListener("pointerup", on_up);
    globalThis.addEventListener("pointercancel", on_up);
    document.addEventListener("visibilitychange", on_visibility);
    globalThis.addEventListener("pagehide", on_page_hide);
    const observer = new ResizeObserver(on_resize);
    observer.observe(element);

    // Recovers from a denied state the moment the user grants location access
    // from outside the app, since nothing else would ask again.
    let permission: PermissionStatus | null = null;
    const on_permission = () => {
      send_permission(Sensor.Geolocation, permission?.state === "granted" ? PermissionAnswer.Granted : PermissionAnswer.Denied);
    };
    navigator.permissions?.query({ name: "geolocation" }).then((status) => {
      permission = status;
      status.addEventListener("change", on_permission);
    }).catch(() => {});

    send_rect(element);
    kernel_command(Command.SENSOR_OPEN);
    strobe.start();

    return () => {
      strobe.stop();
      kernel_command(Command.SENSOR_CLOSE);
      permission?.removeEventListener("change", on_permission);
      observer.disconnect();
      globalThis.removeEventListener("pagehide", on_page_hide);
      document.removeEventListener("visibilitychange", on_visibility);
      globalThis.removeEventListener("pointercancel", on_up);
      globalThis.removeEventListener("pointerup", on_up);
      globalThis.removeEventListener("scroll", on_resize, true);
      globalThis.removeEventListener("resize", on_resize);
      element.removeEventListener("wheel", on_wheel);
      element.removeEventListener("contextmenu", on_context_menu);
      element.removeEventListener("pointerleave", on_leave);
      element.removeEventListener("pointermove", on_move);
      element.removeEventListener("pointerdown", on_down);
      set_driver(null);
    };
  }, [element]);
  return null;
}
