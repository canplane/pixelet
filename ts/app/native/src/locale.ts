// @pxl/native/locale.ts

import { PLATFORM } from "./module.ts";

export function list(): readonly string[] {
  return [Intl.DateTimeFormat().resolvedOptions().locale];
}

export function show(lng: string): void {
  PLATFORM.textLanguage(lng);
}
