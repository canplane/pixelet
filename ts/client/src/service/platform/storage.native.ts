// @pxl/client/service/platform/storage.native.ts

/** UserDefaults or SharedPreferences, read and written synchronously as localStorage is. */

import { PLATFORM } from "./pxl-platform.native.ts";

export function get(key: string): string | null {
  return PLATFORM.storage_get(key);
}

export function keys(): string[] {
  return PLATFORM.storage_keys();
}

export function set(key: string, value: string): void {
  PLATFORM.storage_set(key, value);
}
