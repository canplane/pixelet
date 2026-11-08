// @pxl/client/service/platform/storage.ts

/** The page's small values, in localStorage. Where the browser refuses, as in
 * a sandboxed frame or past a private window's quota, nothing is kept and
 * nothing throws. */

export function get(key: string): string | null {
  try {
    return globalThis.localStorage?.getItem(key) ?? null;
  } catch {
    return null;
  }
}

export function keys(): string[] {
  try {
    return Object.keys(globalThis.localStorage);
  } catch {
    return [];
  }
}

export function set(key: string, value: string): void {
  try {
    globalThis.localStorage?.setItem(key, value);
  } catch {
    // The browser refused the write, so the value lasts only this session.
  }
}
