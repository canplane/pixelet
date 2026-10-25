// @pxl/client/bracket.tsx

/**
 * What the client asks of the place it runs in, as pxl-kernel's door is
 * handed its place by its bracket (pxl_kernel::door `intr`): the page's
 * (ts/app/web) or the device's (ts/app/native), set once by the app's entry
 * before the client boots.
 */

import type { ComponentType } from "react";

import type { f32, f64, i32, u32 } from "@xpute/kit/abi/word.ts";
import type { Guest } from "@xpute/host/host.ts";

export type { Guest };

/** What a place needs to open the guest, which the guest's own numbers say:
 * its memory, the offset its directory is at, and where its GPU stream's
 * records begin. A place that links the guest in may need none of it. */
export interface Plan {
  bytes: u32;
  region: u32;
  directory_at: u32;
  /** The directory's key for the GPU stream, and the stream's head, in words. */
  gpu_stream: u32;
  gpu_head: u32;
}

/** What a place's GPU says, in its words. */
export interface GpuHeard {
  /** A device answered, the first time or after a loss. */
  ready(): void;
  /** Whether there is a GPU: one, none yet while it asks again, none it will ask for again, or none again after one it drew with. A GPU counts as present until a place says otherwise, and a place says only what it hears. */
  state(state: "present" | "waiting" | "missing" | "lost"): void;
  /** A completion the device wrote between turns, which a turn must read. */
  wake(): void;
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
export interface SensorHeard {
  position(lon: f64, lat: f64, altitude: f64 | null, accuracy: f64 | null, course: f64 | null): void;
  positionError(error: "denied" | "unavailable" | "timeout" | "other"): void;
  orientation(compass: f64 | null, alpha: f64 | null, beta: f64 | null, gamma: f64 | null): void;
  orientationLost(): void;
  permission(sensor: "geolocation" | "orientation", answer: "asking" | "granted" | "denied" | "failed"): void;
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
  /** The world opened, asking `viewport` each frame; the returned call closes it. */
  open(viewport: () => Viewport): () => void;
  rect(left: f64, top: f64, width: f64, height: f64): void;
  pointer(phase: PointerPhase, kind: PointerKind, id: i32, button: i32, x: f64, y: f64): void;
  wheel(delta_y: f64): void;
  /** Whether the key was taken, so the page's default is prevented. */
  keyDown(code: string, at: { inField: boolean }): boolean;
  keyUp(code: string): void;
  releaseKeys(): void;
  releasePointers(): void;
  visible(visible: boolean): void;
  /** A turn, for a box that moved without the kernel's asking. */
  requestFrame(): void;
}

export interface WorldSurfaceProps {
  surface: SurfaceDriver;
  input: InputDriver;
  overMarker: boolean;
}

export interface CaseSurfaceProps {
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

/** The CSS variable a page's `view.useVisualViewport` keeps on its root, every frame: how far the
 * visual viewport's bottom stands above the layout viewport's, the keyboard's height, in px. Set by
 * the place and read by the shell's CSS (shell/ui/above-keyboard.tsx); a place without CSS sets none. */
export const VISUAL_VIEWPORT_BOTTOM = "--vv-bottom";

export interface Bracket {
  /** Whether the device is held to a phone's ceilings. */
  device: { handheld: boolean };
  /** The guest opened in this place: its memory and its doorbell. */
  guest: { open(plan: Plan): Promise<Guest> };
  /** Asks for a GPU, and says what it heard of one. */
  gpu: { boot(heard: GpuHeard): void };
  storage: { get(key: string): string | null; set(key: string, value: string): void };
  /** `rgba` is 0xRRGGBBAA; `out` is RGBA, top row first, not premultiplied. */
  text: {
    width(weight: u32, px: f32, text: string): f32;
    raster(weight: u32, px: f32, rgba: u32, text: string, x: f32, width: u32, height: u32, out: Uint8Array): void;
  };
  /** Where a route is asked of ("" on a page, its own origin); `get` rejects with a TypeError on a transport failure. `credits`
   * are the reads the place runs at once, each frame's to the kernel, so its scheduler, not the place's queue, orders the rest. */
  origin: { url: string; credits: u32; get(route: string, signal?: AbortSignal): Promise<Response> };
  /** Whether the device is online, as far as the place can tell: a place that cannot says nothing. */
  online: { watch(report: (online: boolean) => void): void };
  sensor: {
    geo: { has: boolean; start(): void; stop(): void };
    /** `gated`: a permission asked inside a gesture; `silence`: no event is how a loss shows. */
    orient: { has: boolean; gated: boolean; silence: boolean; permit(): Promise<boolean>; start(): void; stop(): void };
    /** Once, at boot, before any sensor starts. */
    hear(heard: SensorHeard): void;
    follow(): () => void;
  };
  share: { save(name: string, text: string, mime: string): Promise<void>; copy(text: string): Promise<void> };
  locale: { list(): readonly string[]; show(lng: string): void };
  /** The place's own long frames, as its platform reports them: `sources`, the entries it hears them by, none where it
   * reports none; `watch`, once, hears each from then on and those it buffered before. */
  longFrame: { sources(): readonly LongFrameSource[]; watch(heard: (frame: LongFrame) => void): void };
  view: {
    useSafeArea(): SafeArea;
    useWindowSize(): { width: f64; height: f64 };
    /** Keeps `VISUAL_VIEWPORT_BOTTOM` while mounted. */
    useVisualViewport(): void;
    useViewportTop(): number;
  };
  /** The world's surface, and the place's input driver. */
  surface: { World: ComponentType<WorldSurfaceProps>; Case: ComponentType<CaseSurfaceProps> };
}

/** The place's bracket. */
export let bracket: Bracket;

/** Once, from the app's entry, before the client boots. */
export function set(given: Bracket): void {
  bracket = given;
}
