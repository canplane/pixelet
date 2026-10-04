// @pxl/web/platform/web/locale.ts

import type { LanguageDetectorModule } from "i18next";
import LanguageDetector from "i18next-browser-languagedetector";

import { storage, STORAGE_KEYS } from "@/kernel/abi/host.spec.ts";

export const languageDetector: LanguageDetectorModule = new LanguageDetector(null, {
  order: ["localStorage", "navigator"],
  caches: ["localStorage"],
  lookupLocalStorage: STORAGE_KEYS[storage.LANG],
});

/** index.html ships a static lang="en" for crawlers. */
export function showLanguage(lng: string): void {
  document.documentElement.lang = lng;
}
