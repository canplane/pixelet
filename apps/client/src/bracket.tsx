// @pxl/client/bracket.tsx

/**
 * What the client asks of the place it runs in, and hands it, as pxl-kernel's door asks
 * its bracket (pxl_kernel::door::Bracket): the page's (apps/web) or the
 * device's (apps/native), handed in once by the app's entry before anything
 * here runs. Nothing here names either, and neither names the other.
 */

import type { ComponentType } from "react";

import type { f32, f64, i32, u32 } from "@xpute/kit/abi/word.ts";
import type { Guest } from "@/host/module.ts";

export type { Guest };

/** What a place needs to open the guest, which the guest's own numbers say:
 * its memory, the offset its directory is at, and where its GPU stream's
 * records begin. A place that links the guest in may need none of it. */
export interface GuestPlan {
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

export interface Bracket {
  /** The guest opened in this place: its memory and its doorbell. */
  openGuest(plan: GuestPlan): Promise<Guest>;
  /** Asks for a GPU, and says what it heard of one. */
  gpuBoot(heard: GpuHeard): void;
  /** Whether the device is held to a phone's ceilings. */
  HANDHELD: boolean;

  storageGet(key: string): string | null;
  storageSet(key: string, value: string): void;

  /** `rgba` is 0xRRGGBBAA; `out` is RGBA, top row first, not premultiplied. */
  textWidth(weight: u32, px: f32, text: string): f32;
  textRaster(weight: u32, px: f32, rgba: u32, text: string, x: f32, width: u32, height: u32, out: Uint8Array): void;

  /** Where a route is asked of: "" on a page, its own origin. */
  ORIGIN: string;
  /** Rejects with a TypeError on a transport failure. */
  originGet(route: string, signal?: AbortSignal): Promise<Response>;
  /** Says whether the device is online, as far as the place can tell: a place that cannot says nothing. */
  onlineWatch(report: (online: boolean) => void): void;

  geolocationSupported(): boolean;
  orientationSupported(): boolean;
  orientationNeedsPermission(): boolean;
  orientationSilenceIsLoss(): boolean;
  orientationPermit(): Promise<boolean>;
  permissionFollow(): () => void;
  geolocationStart(): void;
  geolocationStop(): void;
  orientationListen(): void;
  orientationRemove(): void;
  /** Once, at boot, before any sensor starts. */
  sensorsHear(heard: SensorHeard): void;

  saveFile(name: string, text: string, mime: string): Promise<void>;
  copyText(text: string): Promise<void>;

  preferredLanguages(): readonly string[];
  showLanguage(lng: string): void;

  useSafeArea(): SafeArea;
  useWindowSize(): { width: f64; height: f64 };
  useVisualViewport(): void;
  useViewportTop(): number;

  /** The world's surface, and the place's input driver. */
  WorldSurface: ComponentType<WorldSurfaceProps>;
  CaseSurface: ComponentType<CaseSurfaceProps>;
}

let given: Bracket | null = null;

/** Live bindings, set with the bracket. */
export let HANDHELD = false;
export let ORIGIN = "";

/** Once, from the app's entry, before the client boots. */
export function setBracket(bracket: Bracket): void {
  given = bracket;
  HANDHELD = bracket.HANDHELD;
  ORIGIN = bracket.ORIGIN;
}

function b(): Bracket {
  if (given === null) throw new Error("no bracket: the app's entry calls setBracket before the client runs");
  return given;
}

export const openGuest: Bracket["openGuest"] = (plan) => b().openGuest(plan);
export const gpuBoot: Bracket["gpuBoot"] = (heard) => b().gpuBoot(heard);
export const storageGet: Bracket["storageGet"] = (key) => b().storageGet(key);
export const storageSet: Bracket["storageSet"] = (key, value) => b().storageSet(key, value);
export const textWidth: Bracket["textWidth"] = (weight, px, text) => b().textWidth(weight, px, text);
export const textRaster: Bracket["textRaster"] = (...args) => b().textRaster(...args);
export const originGet: Bracket["originGet"] = (route, signal) => b().originGet(route, signal);
export const onlineWatch: Bracket["onlineWatch"] = (report) => b().onlineWatch(report);
export const geolocationSupported: Bracket["geolocationSupported"] = () => b().geolocationSupported();
export const orientationSupported: Bracket["orientationSupported"] = () => b().orientationSupported();
export const orientationNeedsPermission: Bracket["orientationNeedsPermission"] = () => b().orientationNeedsPermission();
export const orientationSilenceIsLoss: Bracket["orientationSilenceIsLoss"] = () => b().orientationSilenceIsLoss();
export const orientationPermit: Bracket["orientationPermit"] = () => b().orientationPermit();
export const permissionFollow: Bracket["permissionFollow"] = () => b().permissionFollow();
export const geolocationStart: Bracket["geolocationStart"] = () => b().geolocationStart();
export const geolocationStop: Bracket["geolocationStop"] = () => b().geolocationStop();
export const orientationListen: Bracket["orientationListen"] = () => b().orientationListen();
export const orientationRemove: Bracket["orientationRemove"] = () => b().orientationRemove();
export const sensorsHear: Bracket["sensorsHear"] = (heard) => b().sensorsHear(heard);
export const saveFile: Bracket["saveFile"] = (name, text, mime) => b().saveFile(name, text, mime);
export const copyText: Bracket["copyText"] = (text) => b().copyText(text);
export const preferredLanguages: Bracket["preferredLanguages"] = () => b().preferredLanguages();
export const showLanguage: Bracket["showLanguage"] = (lng) => b().showLanguage(lng);
export const useSafeArea: Bracket["useSafeArea"] = () => b().useSafeArea();
export const useWindowSize: Bracket["useWindowSize"] = () => b().useWindowSize();
export const useVisualViewport: Bracket["useVisualViewport"] = () => b().useVisualViewport();
export const useViewportTop: Bracket["useViewportTop"] = () => b().useViewportTop();

export function WorldSurface(props: WorldSurfaceProps) {
  const Surface = b().WorldSurface;
  return <Surface {...props} />;
}

export function CaseSurface(props: CaseSurfaceProps) {
  const Surface = b().CaseSurface;
  return <Surface {...props} />;
}
