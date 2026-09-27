// @pxl/web/platform/web/storage.ts

/**
 * What the app keeps across loads, by key: the page's localStorage. Storage
 * can refuse either way — none in a sandboxed frame, a quota in a private
 * window — and a refusal is nothing kept, never an error: what was not
 * written lasts the session.
 */

/** What is kept under `key`, or none. */
export function storageGet(key: string): string | null {
  try {
    return globalThis.localStorage?.getItem(key) ?? null;
  } catch {
    return null;
  }
}

/** Keeps `value` under `key`. */
export function storageSet(key: string, value: string): void {
  try {
    globalThis.localStorage?.setItem(key, value);
  } catch {
    // Refused: the value lasts the session.
  }
}
