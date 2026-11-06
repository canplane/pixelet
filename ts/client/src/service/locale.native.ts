// @pxl/client/service/locale.native.ts

import { PLATFORM } from "./pxl-platform.ts";

export function list(): readonly string[] {
  return [Intl.DateTimeFormat().resolvedOptions().locale];
}

export function show(lng: string): void {
  PLATFORM.textLanguage(lng);
}
