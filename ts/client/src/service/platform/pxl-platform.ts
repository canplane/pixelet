// @pxl/client/service/platform/pxl-platform.ts

/** The app's own native module (PlatformModule.swift, PlatformModule.kt under
 * modules/platform/), beside xpute's (@xpute/native-runtime). */

import type { f64 } from "@xpute/kit/abi/word.ts";
import { requireNativeModule } from "expo";

interface PxlPlatform {
  /** Decodes a body sent zstd-compressed, off the JavaScript thread
   * (origin.native.ts). */
  unpack(packed: Uint8Array): Promise<ArrayBuffer>;
  storage_get(key: string): string | null;
  storage_set(key: string, val: string): void;
  clipboard_set(text: string): void;
  has_heading(): boolean;
  /** The part of the bottom inset that takes a touch, in dp: Android's bar of
   * navigation buttons. It is 0 under gesture navigation and on iOS. */
  tappable_bottom(): f64;
}

export const PLATFORM: PxlPlatform =
  requireNativeModule<PxlPlatform>("PxlPlatform");
