// @pxl/client/service/platform/pxl-platform.ts

/** The app's own native module (PlatformModule.swift, PlatformModule.kt under
 * modules/platform/), beside xpute's (@xpute/native-runtime). */

import type { f64 } from "@xpute/kit/abi/word.ts";
import { requireNativeModule } from "expo";

interface PxlPlatform {
  /** Decodes a body sent zstd-compressed, off the JavaScript thread
   * (origin.native.ts). */
  unpack(packed: Uint8Array): Promise<ArrayBuffer>;
  storageGet(key: string): string | null;
  storageSet(key: string, val: string): void;
  clipboardSet(text: string): void;
  headingAvailable(): boolean;
  /** The part of the bottom inset that takes a touch, in dp: Android's bar of
   * navigation buttons. It is 0 under gesture navigation and on iOS. */
  tappableBottom(): f64;
}

export const PLATFORM: PxlPlatform =
  requireNativeModule<PxlPlatform>("PxlPlatform");
