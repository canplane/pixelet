// @pxl/web/storage.ts

/** localStorage; a refusal (sandboxed frame, private-window quota) keeps nothing, never throws. */

export function storageGet(key: string): string | null {
  try {
    return globalThis.localStorage?.getItem(key) ?? null;
  } catch {
    return null;
  }
}

export function storageSet(key: string, value: string): void {
  try {
    globalThis.localStorage?.setItem(key, value);
  } catch {
    // Refused: the value lasts the session.
  }
}
