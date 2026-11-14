// @pxl/client/service/platform/types.ts

/**
 * The shapes the platform's services share. Each service is a pair of files,
 * x.ts for the page and x.native.ts for a device, and what both give and take
 * is named here. A place, in these names, is where the client runs: the page
 * or a device.
 */

import type { U8Array } from "@xpute/kit/abi/array.ts";
import type { f64, i32, u32 } from "@xpute/kit/abi/word.ts";

/** A transaction on the place's database, which keeps files of bytes by key.
 * null stands for no file. */
export interface Tx {
  get(key: string): Promise<U8Array | null>;
  put(key: string, bytes: U8Array | null): Promise<void>;
}

export interface SafeArea {
  top: number;
  bottom: number;
  /** The part of `bottom` that takes a touch, such as Android's navigation
   * buttons. A gesture area is never counted. */
  tappable: number;
}

export interface Viewport {
  width: f64;
  height: f64;
  dpr: f64;
}

/** A position as the page's API and a device's both give it. */
export interface Coords {
  longitude: f64;
  latitude: f64;
  altitude: f64 | null;
  altitudeAccuracy: f64 | null;
  accuracy: f64 | null;
  heading: f64 | null;
}

/** A position fix. */
export interface Fix {
  lon: f64;
  lat: f64;
  alt: f64 | null;
  alt_accuracy: f64 | null;
  accuracy: f64 | null;
  heading: f64 | null;
}

export interface SensorListener {
  pos(
    lon: f64,
    lat: f64,
    alt: f64 | null,
    accuracy: f64 | null,
    course: f64 | null,
  ): void;
  /** `watching` says whether the watch runs on past the failure, which only
   * the platform knows: a page's watch rides out all but a refusal, and a
   * device's ends on any. */
  pos_err(
    err: "denied" | "unavailable" | "timeout" | "other",
    watching: boolean,
  ): void;
  orient(compass: f64 | null, alpha: f64 | null): void;
  /** The place has no compass: a page's event with every field null. */
  orient_unavailable(): void;
  orient_err(): void;
  permission(
    sensor: "geolocation" | "orient",
    answer: "asking" | "granted" | "denied" | "failed",
  ): void;
}

export interface GeolocationSensor {
  has: boolean;
  /** Called from a press, before it starts the watch. Resolves whether the
   * watch may start: it asks again where a refusal may be asked again, and
   * where it may not, opens the settings where the user grants it. */
  permit(): Promise<boolean>;
  start(): void;
  stop(): void;
}

export interface OrientationSensor {
  has: boolean;
  /** Whether a permission prompt, which must be raised inside a gesture,
   * gates it, as iOS Safari's does. */
  gated: boolean;
  /** Whether its loss shows only as silence. A browser's compass says nothing
   * when it stops hearing, while a device's reports its failure. */
  silence: boolean;
  /** Resolves whether it is granted, asking first where a prompt gates it. */
  permit(): Promise<boolean>;
  start(): void;
  stop(): void;
}

/** Handle 0 is no surface. */
export interface SurfaceDriver {
  /** Asks the core for a new surface's handle, which is 0 when the core has
   * none left. */
  open(): Promise<u32>;
  size(handle: u32, width: u32, height: u32, dpr: f64): void;
  /** Closes the surface, which takes back the canvas `bind` lent. */
  close(handle: u32): void;
  /** Lends a page's canvas to the GPU under `handle`, until `close`. */
  bind(handle: u32, canvas: HTMLCanvasElement): void;
}

export type PointerPhase = "down" | "move" | "up" | "cancel" | "out";
export type PointerKind = "mouse" | "touch" | "pen";

export interface InputDriver {
  /** Called when the world's surface opens. The function it returns is
   * called when the surface closes. */
  open(): () => void;
  /** A pointer event. `x` and `y` are in CSS pixels from the top left of the
   * canvas's layout box, without its transforms. */
  pointer(
    phase: PointerPhase,
    kind: PointerKind,
    id: i32,
    button: i32,
    x: f64,
    y: f64,
  ): void;
  wheel(delta_y: f64): void;
  /** A trackpad's pinch: the factor its fingers' spread changed by since the
   * last, over one as they spread. */
  pinch(scale: f64): void;
  /** Returns whether the core took the key, in which case the page's default
   * action is prevented. */
  key_down(code: string, at: { in_field: boolean }): boolean;
  key_up(code: string): void;
  release_keys(): void;
  release_pointers(): void;
}

export interface WorldSurfaceProps {
  surface: SurfaceDriver;
  input: InputDriver;
  overMarker: boolean;
}

export interface CharacterSurfaceProps {
  surface: SurfaceDriver;
  onOpen: (surface: u32) => () => void;
  openKey: string;
}

/** The performance entry that reports a long frame: a long animation frame
 * (`loaf`) or a long task. */
export type LongFrameSource = "loaf" | "longtask";

/** A long frame of the place's own. Its times are on the clock of
 * `performance.now()`, which the core's frames use too. */
export interface LongFrame {
  source: LongFrameSource;
  t: f64;
  duration_ms: f64;
  blocking_ms: f64;
  script_ms: f64;
  render_ms: f64;
}

/** The CSS variable that a page's `useVisualViewport` keeps on its root on
 * every frame. It holds how far the visual viewport's bottom stands above the
 * layout viewport's, which is the keyboard's height, in px. The page sets it
 * (service/platform/viewport.ts) and the UI's CSS reads it
 * (ui/shared/component/above-keyboard.tsx). A device never sets it. */
export const VISUAL_VIEWPORT_BOTTOM = "--vv-bottom";
