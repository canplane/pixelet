// @pxl/native/platform/module.ts

/** The app's one native module (modules/platform/ios/PlatformModule.swift). */

import type { f32, f64, u32 } from "@xpute/core/abi/word.ts";
import { requireNativeModule } from "expo";

interface PxlPlatform {
  memory(): ArrayBuffer;
  interrupt(quotaMs: f64): f64;
  /** Uniform offset alignment and whether it times passes; null with no GPU. */
  gpuOpen(): [u32, u32] | null;
  /** Decodes off the JS thread; 1, or 0 when it does not decode. */
  imageHold(handle: u32, file: Uint8Array): Promise<u32>;
  storageGet(key: string): string | null;
  storageSet(key: string, value: string): void;
  clipboardSet(text: string): void;
  textWidth(weight: u32, px: f32, text: string): f32;
  textRaster(weight: u32, px: f32, rgba: u32, text: string, x: f32, width: u32, height: u32, out: Uint8Array): void;
  headingAvailable(): boolean;
  /** Android's: the bottom inset that takes a touch (nav buttons), 0 with gestures, in dp. */
  tappableBottom(): f64;
}

export const PLATFORM: PxlPlatform = requireNativeModule<PxlPlatform>("PxlPlatform");
