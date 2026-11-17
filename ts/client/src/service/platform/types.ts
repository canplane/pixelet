// @pxl/client/service/platform/types.ts

/**
 * The shapes the platform's services share. Each service is a pair of files,
 * x.ts for the web and x.native.ts for native, and what both give and take is
 * named here. A place, in these names, is where the client runs: the web or a
 * native app.
 */

import type { U8Array } from "@xpute/kit/abi/array.ts";
import type { f64, u32 } from "@xpute/kit/abi/word.ts";

/** A transaction on the place's database, which keeps files of bytes by key.
 * null stands for no file. */
export interface Transaction {
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

/** A position as the web API and the native one both give it. */
export interface Coordinates {
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
  altitudeAccuracy: f64 | null;
  accuracy: f64 | null;
  heading: f64 | null;
}

/** Why a position's watch failed. */
export type GeolocationError = "denied" | "unavailable" | "timeout" | "other";

/** A permission's answer. */
type PermissionAnswer = "granted" | "denied" | "failed";

export interface SensorListener {
  pos(
    lon: f64,
    lat: f64,
    alt: f64 | null,
    accuracy: f64 | null,
    course: f64 | null,
  ): void;
  /** `watching` says whether the watch runs on past the failure, which only
   * the platform knows: a web watch rides out all but a refusal, and a
   * native one ends on any. */
  positionError(error: GeolocationError, watching: boolean): void;
  /** A heading, in degrees clockwise from north, or null for a reading with
   * no heading. */
  orientation(heading: f64 | null): void;
  /** The place has no compass: a web event with every field null. */
  orientationUnavailable(): void;
  orientationError(): void;
  permission(
    sensor: "geolocation" | "orientation",
    answer: PermissionAnswer,
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
  /** Whether its loss shows only as silence. A browser's compass says nothing
   * when it stops hearing, while a native one reports its failure. */
  silence: boolean;
  /** Resolves whether it is granted, from the decision the place holds. Where
   * none is held, a prompt is raised only inside a press, as iOS Safari's must
   * be, and it resolves false elsewhere. A refusal is also told the listener
   * (`permission`). */
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
  /** Closes the surface, which takes back a web canvas lent under it. */
  close(handle: u32): void;
}

/** What the world's surface hears that is not a gesture: the wheel, a
 * trackpad's pinch on the web and the keys. The gestures are recognized
 * above the surface (ui/feature/world/gesture.tsx). */
export interface InputDriver {
  wheel(deltaY: f64): void;
  /** A trackpad's pinch: the factor its fingers' spread changed by since the
   * last, over one as they spread. */
  pinch(scale: f64): void;
  /** Returns whether the core took the key, in which case the page's default
   * action is prevented. */
  keyDown(code: string, at: { inField: boolean }): boolean;
  keyUp(code: string): void;
  releaseKeys(): void;
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
  durationMs: f64;
  blockingMs: f64;
  scriptMs: f64;
  renderMs: f64;
}

/** The CSS variable that the web `useVisualViewport` keeps on its root on
 * every frame. It holds how far the visual viewport's bottom stands above the
 * layout viewport's, which is the keyboard's height, in px. The web file sets
 * it (service/platform/viewport.web.ts) and the UI's CSS reads it
 * (ui/shared/component/above-keyboard.web.tsx). Natively nothing sets it. */
export const VISUAL_VIEWPORT_BOTTOM = "--vv-bottom";
