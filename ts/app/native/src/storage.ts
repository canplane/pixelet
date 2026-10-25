// @pxl/native/storage.ts

/** UserDefaults or SharedPreferences, read and written synchronously as localStorage is. */

import { PLATFORM } from "./module.ts";

export function get(key: string): string | null {
  return PLATFORM.storageGet(key);
}

export function set(key: string, value: string): void {
  PLATFORM.storageSet(key, value);
}
