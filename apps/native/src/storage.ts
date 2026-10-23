// @pxl/native/storage.ts

/** UserDefaults or SharedPreferences, read and written synchronously as localStorage is. */

import { PLATFORM } from "./module.ts";

export function storageGet(key: string): string | null {
  return PLATFORM.storageGet(key);
}

export function storageSet(key: string, value: string): void {
  PLATFORM.storageSet(key, value);
}
