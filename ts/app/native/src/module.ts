// @pxl/native/module.ts

/** The app's one native module (PlatformModule.swift, PlatformModule.kt under modules/platform/). */

import type { f32, f64, u32 } from "@xpute/kit/abi/word.ts";
import { requireNativeModule } from "expo";

interface PxlPlatform {
  /** The module's event (Expo's EventEmitter): `onWake`, the device's host
   * asking a turn (gpu.ts). */
  addListener(event: "onWake", listener: () => void): { remove(): void };
  mem(): ArrayBuffer;
  intr(quotaMs: f64): f64;
  /** Whether the device opened; what it is, the kernel reads from the host. */
  gpuOpen(): boolean;
  /** A body sent zstd, decoded off the JavaScript thread (origin.ts). */
  unpack(packed: Uint8Array): Promise<ArrayBuffer>;
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
