// @pxl/client/services/types.ts

/**
 * The shapes the client's services share: each service is a file a platform
 * has of its own beside it (x.ts the page's, x.native.ts the device's), and
 * what both give and take is named here.
 */

import type { U8Array } from "@xpute/kit/abi/array.ts";
import type { f64, i32, u32 } from "@xpute/kit/abi/word.ts";
import type { Gpu as HostGpu } from "@xpute/host";
import type { GpuState } from "@xpute/gpu/web.ts";

/** A transaction on the place's database: files of bytes by key, null none. */
export interface Tx {
  get(key: string): Promise<U8Array | null>;
  put(key: string, bytes: U8Array | null): Promise<void>;
}

/** What a place needs to open the guest: the bytes of its memory, one of
 * those its machine allows. A place that links the guest in needs none. */
export interface Config {
  bytes: u32;
}

/** A place's GPU, which xpute grants the kernel as its machine asks
 * (@xpute/gpu `WebGpu`, `NativeGpu`); `watch` tells whether there is one. A
 * GPU counts as present until a place says otherwise, and a place says only
 * what it hears. */
export interface Gpu extends HostGpu {
  watch(report: (state: GpuState) => void): void;
}

export interface SafeArea {
  top: number;
  bottom: number;
  /** The part of `bottom` that takes a touch: Android's navigation buttons, never a gesture area. */
  tappable: number;
}

export interface Viewport {
  width: f64;
  height: f64;
  dpr: f64;
}

/** What the sensors hear, in the platform's words: handed in once, at boot. */
export interface SensorListener {
  position(
    lon: f64,
    lat: f64,
    altitude: f64 | null,
    accuracy: f64 | null,
    course: f64 | null,
  ): void;
  positionError(error: "denied" | "unavailable" | "timeout" | "other"): void;
  orientation(
    compass: f64 | null,
    alpha: f64 | null,
    beta: f64 | null,
    gamma: f64 | null,
  ): void;
  orientationLost(): void;
  permission(
    sensor: "geolocation" | "orientation",
    answer: "asking" | "granted" | "denied" | "failed",
  ): void;
}

/** A surface the guest draws into, by the handle it gave; 0 is none. */
export interface SurfaceDriver {
  open(): u32;
  size(handle: u32, width: u32, height: u32, dpr: f64): void;
  close(handle: u32): void;
}

export type PointerPhase = "down" | "move" | "up" | "cancel" | "out";
export type PointerKind = "mouse" | "touch" | "pen";

/** What the place's input says, in its words. */
export interface InputDriver {
  /** The world opened; the returned call closes it. */
  open(): () => void;
  /** A pointer, `x` and `y` in CSS pixels from the canvas's top left, as its
   * layout box is laid out (transforms aside). */
  pointer(
    phase: PointerPhase,
    kind: PointerKind,
    id: i32,
    button: i32,
    x: f64,
    y: f64,
  ): void;
  wheel(delta_y: f64): void;
  /** Whether the key was taken, so the page's default is prevented. */
  keyDown(code: string, at: { inField: boolean }): boolean;
  keyUp(code: string): void;
  releaseKeys(): void;
  releasePointers(): void;
  /** A turn, for a box that moved without the kernel's asking. */
  requestFrame(): void;
}

export interface WorldSurfaceProps {
  surface: SurfaceDriver;
  input: InputDriver;
  overMarker: boolean;
}

export interface PreviewSurfaceProps {
  surface: SurfaceDriver;
  onOpen: (surface: u32) => () => void;
  openKey: string;
}

/** The entry a place hears a long frame by: a long animation frame, or a long task. */
export type LongFrameSource = "loaf" | "longtask";

/** A long frame of the place's own, on `performance.now()`'s clock, which the kernel's frames are on too. */
export interface LongFrame {
  source: LongFrameSource;
  t: f64;
  durationMs: f64;
  blockingMs: f64;
  scriptMs: f64;
  renderMs: f64;
}

/** The CSS variable a page's `useVisualViewport` keeps on its root, every frame: how far the
 * visual viewport's bottom stands above the layout viewport's, the keyboard's height, in px. Set by
 * the page (services/viewport.ts) and read by the shell's CSS (shell/ui/above-keyboard.tsx); the device sets none. */
export const VISUAL_VIEWPORT_BOTTOM = "--vv-bottom";
