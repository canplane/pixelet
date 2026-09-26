// @pxl/web/platform/web/locale.ts

import type { LanguageDetectorModule } from "i18next";
import LanguageDetector from "i18next-browser-languagedetector";

/** Which language the app opens in: the one last picked, kept in the
 * browser's storage, and otherwise the browser's own. */
export const languageDetector: LanguageDetectorModule = new LanguageDetector(null, {
  order: ["localStorage", "navigator"],
  caches: ["localStorage"],
  lookupLocalStorage: "pxl:lang",
});

/** Says which language the page is in. index.html ships a static lang="en"
 * for crawlers, which never run the bundle that would otherwise correct it;
 * for real visitors it follows whatever i18next resolves and switches to. */
export function showLanguage(lng: string): void {
  document.documentElement.lang = lng;
}
