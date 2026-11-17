// @pxl/client/service/platform/locale.native.ts

export function list(): readonly string[] {
  return [Intl.DateTimeFormat().resolvedOptions().locale];
}

/** The native app has no document whose language a reader would be told, so
 * this does nothing. */
export function show(_lng: string): void {}
