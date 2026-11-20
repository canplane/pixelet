// @pxl/client/service/core/input.ts

import type { f64, i32 } from "@xpute/kit/abi/word.ts";
import * as input from "@/abi/input.ts";
import { ui } from "@/service/core/core.ts";
import { ListenerSet } from "@xpute/kit/lang/listener.ts";

/** The log of the zoom one CSS pixel of a wheel asks for: a wheel turned down
 * zooms out. */
const WHEEL_GAIN = 0.0022;

/** What the hand does with the world from its press to its release, as the
 * UI's gestures recognize it (ui/feature/world/gesture.tsx): it pans, rotates
 * or pinches. */
export type DragMode = "none" | "pan" | "rotate" | "pinch";

let mode: DragMode = "none";
const modeListeners = new ListenerSet();

/** The core hears whether a hand drags the view. */
export function setDragMode(next: DragMode): void {
  if (next === mode) return;
  if ((next === "none") !== (mode === "none"))
    void input.drag(ui, next !== "none");
  mode = next;
  modeListeners.notify();
}

export function dragMode(): DragMode {
  return mode;
}

export const watchDragMode = modeListeners.watch;

/** Told of each zoom; one of one zooms nothing. */
const zoomListeners = new ListenerSet();

export const watchZoom = zoomListeners.watch;

export function zoom(scale: f64): void {
  if (scale !== 1) zoomListeners.notify();
  void input.zoom(ui, scale);
}

/** A wheel turn of `deltaY` CSS pixels. */
export function wheel(deltaY: f64): void {
  zoom(Math.exp(-deltaY * WHEEL_GAIN));
}

export function pan(dx: f64, dy: f64): void {
  void input.pan(ui, dx, dy);
}

export function rotate(radians: f64): void {
  void input.rotate(ui, radians);
}

export function tilt(radians: f64): void {
  void input.tilt(ui, radians);
}

export function tap(x: f64, y: f64): void {
  void input.tap(ui, x, y);
}

export function hover(x: f64, y: f64): void {
  void input.hover(ui, x, y);
}

export function out(): void {
  void input.out(ui);
}

export function move(x: i32, y: i32): void {
  void input.move(ui, x, y);
}
