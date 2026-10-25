// @pxl/native/locale.ts

export function list(): readonly string[] {
  return [Intl.DateTimeFormat().resolvedOptions().locale];
}

export function show(_lng: string): void {}
