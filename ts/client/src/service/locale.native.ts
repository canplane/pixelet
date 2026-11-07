// @pxl/client/service/locale.native.ts

export function list(): readonly string[] {
  return [Intl.DateTimeFormat().resolvedOptions().locale];
}

/** A device's app has no document whose language a reader is told. */
export function show(_lng: string): void {}
