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
import { Command } from "@/kernel/abi/cmd.spec.ts";
import { GEOLOCATION, PERMISSION_DENIED, PERMISSION_GRANTED, send_permission } from "@/kernel/control/sensor.ts";
import { CORE_BUILD, packet } from "@/kernel/ipc.ts";
import { request_frame, set_driver, strobe } from "@/kernel/sched.ts";
import { kernel_action } from "@/kernel/shell.ts";

export function send_rect(element: HTMLElement): void {
  const r = element.getBoundingClientRect();
  CORE_BUILD.send(0, Command.INPUT_UPDATE, packet((b) => b.f64(r.left).f64(r.top).f64(r.width).f64(r.height)));
}

/** What the pointer holds: a tool, the placeable cube's or decor's catalog
 * number, and the object size's tier. The kernel keeps no pick of its own; each
 * pointer event carries this. */
interface Holding {
  tool: u32;
  value: u32;
  tier: u32;
}

let holding: Holding = { tool: 0, value: 0, tier: 0 };
/** The last hovering pointer, while it is over the canvas. */
let last_hover: { kind: u32; id: i32; button: i32; x: f64; y: f64 } | null = null;

function send_pointer_at(phase: u32, kind: u32, id: i32, button: i32, x: f64, y: f64): void {
  CORE_BUILD.send(0, Command.INPUT_POINTER, packet((b) => b.u32(phase).u32(kind).i32(id).i32(button).f64(x).f64(y).u32(holding.tool).u32(holding.value).u32(holding.tier)));
  strobe.input();
  request_frame();
}

function send_pointer(phase: u32, e: PointerEvent): void {
  const kind = e.pointerType === "touch" ? 1 : e.pointerType === "pen" ? 2 : 0;
  last_hover = phase === 3 || kind === 1 ? null : { kind, id: e.pointerId, button: e.button, x: e.clientX, y: e.clientY };
  send_pointer_at(phase, kind, e.pointerId, e.button, e.clientX, e.clientY);
}

/**
 * Sets what the pointer holds. A hover under a still pointer would otherwise
 * show the old pick until the pointer moves, so a move at the last position
 * is sent, as a browser synthesizes a mouse move when what is under a still
 * cursor changes.
 */
export function kernel_hold(tool: u32, value: u32, tier: u32): void {
  holding = { tool, value, tier };
  if (last_hover) send_pointer_at(1, last_hover.kind, last_hover.id, last_hover.button, last_hover.x, last_hover.y);
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
      send_pointer(0, e);
    };
    const on_move = (e: PointerEvent) => send_pointer(1, e);
    const on_up = (e: PointerEvent) => send_pointer(2, e);
    const on_leave = (e: PointerEvent) => send_pointer(3, e);
    const on_wheel = (e: WheelEvent) => {
      if (e.ctrlKey) e.preventDefault();
      CORE_BUILD.send(0, Command.INPUT_WHEEL, packet((b) => b.f64(e.deltaY)));
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
      CORE_BUILD.send(0, Command.SENSOR_VISIBILITY, packet((b) => b.bool(visible)));
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
      send_permission(GEOLOCATION, permission?.state === "granted" ? PERMISSION_GRANTED : PERMISSION_DENIED);
    };
    navigator.permissions?.query({ name: "geolocation" }).then((status) => {
      permission = status;
      status.addEventListener("change", on_permission);
    }).catch(() => {});

    send_rect(element);
    kernel_action({ type: "sensors_open" });
    strobe.start();

    return () => {
      strobe.stop();
      kernel_action({ type: "sensors_close" });
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
