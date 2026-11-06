// @pxl/client/service/pxl-platform.ts

/** The app's own native module (PlatformModule.swift, PlatformModule.kt under
 * modules/platform/), beside xpute's (@xpute/native). */

import type { f32, f64, u32 } from "@xpute/kit/abi/word.ts";
import { requireNativeModule } from "expo";

interface PxlPlatform {
  /** A body sent zstd, decoded off the JavaScript thread (origin.ts). */
  unpack(packed: Uint8Array): Promise<ArrayBuffer>;
  storageGet(key: string): string | null;
  storageSet(key: string, value: string): void;
  clipboardSet(text: string): void;
  /** The language text is drawn in, which picks a Han character's form. */
  textLanguage(tag: string): void;
  textWidth(weight: u32, px: f32, text: string): f32;
  textRaster(
    weight: u32,
    px: f32,
    rgba: u32,
    text: string,
    x: f32,
    width: u32,
    height: u32,
    out: Uint8Array,
  ): void;
  headingAvailable(): boolean;
  /** Android's: the bottom inset that takes a touch (nav buttons), 0 with gestures, in dp. */
  tappableBottom(): f64;
}

export const PLATFORM: PxlPlatform =
  requireNativeModule<PxlPlatform>("PxlPlatform");
