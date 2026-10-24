// @pxl/client/bracket.tsx

/**
 * What the client asks of the place it runs in, as pxl-kernel's door asks
 * its bracket (pxl_kernel::door::Bracket): the page's (apps/web) or the
 * device's (apps/native), handed in once by the app's entry before anything
 * here runs. Nothing here names either, and neither names the other.
 */

import type { ComponentType } from "react";

import type { f32, f64, u32 } from "@xpute/kit/abi/word.ts";
import type { Guest } from "@/host/module.ts";
import type { SafeArea } from "@/shell/ui/bottom-rest.ts";

export interface Bracket {
  /** The kernel opened in this place: its memory and its doorbell. */
  openGuest(): Promise<Guest>;
  /** Asks for a GPU and tells the kernel when one answered. */
  gpuBoot(): void;
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
  onlineWatch(): void;

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

  saveFile(name: string, text: string, mime: string): Promise<void>;
  copyText(text: string): Promise<void>;

  preferredLanguages(): readonly string[];
  showLanguage(lng: string): void;

  useSafeArea(): SafeArea;
  useWindowSize(): { width: f64; height: f64 };
  useVisualViewport(): void;
  useViewportTop(): number;

  /** The world's surface, and the place's input driver. */
  WorldSurface: ComponentType;
  CaseSurface: ComponentType<{ onOpen: (surface: u32) => () => void; openKey: string }>;
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

export const openGuest: Bracket["openGuest"] = () => b().openGuest();
export const gpuBoot: Bracket["gpuBoot"] = () => b().gpuBoot();
export const storageGet: Bracket["storageGet"] = (key) => b().storageGet(key);
export const storageSet: Bracket["storageSet"] = (key, value) => b().storageSet(key, value);
export const textWidth: Bracket["textWidth"] = (weight, px, text) => b().textWidth(weight, px, text);
export const textRaster: Bracket["textRaster"] = (...args) => b().textRaster(...args);
export const originGet: Bracket["originGet"] = (route, signal) => b().originGet(route, signal);
export const onlineWatch: Bracket["onlineWatch"] = () => b().onlineWatch();
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
export const saveFile: Bracket["saveFile"] = (name, text, mime) => b().saveFile(name, text, mime);
export const copyText: Bracket["copyText"] = (text) => b().copyText(text);
export const preferredLanguages: Bracket["preferredLanguages"] = () => b().preferredLanguages();
export const showLanguage: Bracket["showLanguage"] = (lng) => b().showLanguage(lng);
export const useSafeArea: Bracket["useSafeArea"] = () => b().useSafeArea();
export const useWindowSize: Bracket["useWindowSize"] = () => b().useWindowSize();
export const useVisualViewport: Bracket["useVisualViewport"] = () => b().useVisualViewport();
export const useViewportTop: Bracket["useViewportTop"] = () => b().useViewportTop();

export function WorldSurface() {
  const Surface = b().WorldSurface;
  return <Surface />;
}

export function CaseSurface(props: { onOpen: (surface: u32) => () => void; openKey: string }) {
  const Surface = b().CaseSurface;
  return <Surface {...props} />;
}
