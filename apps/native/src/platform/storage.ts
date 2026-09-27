// @pxl/native/platform/storage.ts

/**
 * What the app keeps across launches, by key: the device's own key-value
 * store, as a page's localStorage is the browser's (module.ts — UserDefaults
 * on iOS). Read and written at once, on the
 * JavaScript thread, as localStorage is.
 */

import { PLATFORM } from "./module.ts";

/** What is kept under `key`, or none. */
export function storageGet(key: string): string | null {
  return PLATFORM.storageGet(key);
}

/** Keeps `value` under `key`. */
export function storageSet(key: string, value: string): void {
  PLATFORM.storageSet(key, value);
}
