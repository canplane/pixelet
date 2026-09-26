// @pxl/native/platform/module.ts

/**
 * The device's half of this platform: the app's one native module
 * (modules/platform/ios/PlatformModule.swift), which every module here calls
 * — the kernel's memory, doorbell, device and images, storage, the clipboard,
 * text, and the compass. Named in one place so that what it offers is said once.
 */

import type { f32, f64, u32 } from "@xpute/core/abi/word.ts";
import { requireNativeModule } from "expo";

interface PxlPlatform {
  /** The kernel's memory, as one buffer over the region the core took. */
  memory(): ArrayBuffer;
  interrupt(quota_ms: f64): f64;
  /** The device's uniform offset alignment and whether it times its passes,
   * or null where the system has no GPU. */
  gpuOpen(): [u32, u32] | null;
  /** Decodes a PNG or a WebP off the JavaScript thread and holds it for the
   * renderer: its handle, or 0 when it does not decode. */
  imageHold(file: Uint8Array): Promise<u32>;
  storageGet(key: string): string | null;
  storageSet(key: string, value: string): void;
  clipboardSet(text: string): void;
  textWidth(weight: u32, px: f32, text: string): f32;
  textRaster(weight: u32, px: f32, rgba: u32, text: string, x: f32, width: u32, height: u32, out: Uint8Array): void;
  headingAvailable(): boolean;
}

export const PLATFORM: PxlPlatform = requireNativeModule<PxlPlatform>("PxlPlatform");
