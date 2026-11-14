// @pxl/client/service/platform/storage.native.ts

/** UserDefaults or SharedPreferences, read and written synchronously as localStorage is. */

import { PLATFORM } from "./pxl-platform.ts";

export function get(key: string): string | null {
  return PLATFORM.storage_get(key);
}

export function set(key: string, val: string): void {
  PLATFORM.storage_set(key, val);
}
