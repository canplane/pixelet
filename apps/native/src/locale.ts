// @pxl/native/locale.ts

export function preferredLanguages(): readonly string[] {
  return [Intl.DateTimeFormat().resolvedOptions().locale];
}

export function showLanguage(_lng: string): void {}
