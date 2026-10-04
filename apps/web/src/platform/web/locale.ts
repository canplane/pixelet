// @pxl/web/platform/web/locale.ts

import type { LanguageDetectorModule } from "i18next";
import LanguageDetector from "i18next-browser-languagedetector";

export const languageDetector: LanguageDetectorModule = new LanguageDetector(null, {
  order: ["localStorage", "navigator"],
  caches: ["localStorage"],
  lookupLocalStorage: "pxl:lang",
});

/** index.html ships a static lang="en" for crawlers. */
export function showLanguage(lng: string): void {
  document.documentElement.lang = lng;
}
