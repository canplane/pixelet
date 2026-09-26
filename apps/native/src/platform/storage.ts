// @pxl/native/platform/storage.ts

/**
 * What the app keeps across launches, by key: the device's own key-value
 * store, as a page's localStorage is the browser's (modules/pxl-surface's
 * PxlStorage — UserDefaults on iOS). Read and written at once, on the
 * JavaScript thread, as localStorage is.
 */

import { requireNativeModule } from "expo";

interface PxlStorage {
  get(key: string): string | null;
  set(key: string, value: string): void;
}

const STORAGE: PxlStorage = requireNativeModule("PxlStorage");

/** What is kept under `key`, or none. */
export function storage_get(key: string): string | null {
  return STORAGE.get(key);
}

/** Keeps `value` under `key`. */
export function storage_set(key: string, value: string): void {
  STORAGE.set(key, value);
}
