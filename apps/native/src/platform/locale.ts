// @pxl/native/platform/locale.ts

import type { LanguageDetectorModule } from "i18next";

import { storageGet, storageSet } from "./storage.ts";

/** Where a pick is kept, the key the page's detector keeps it under. */
const KEY = "pxl:lang";

/** Which language the app opens in: the one last picked, and otherwise the
 * device's own. */
export const languageDetector: LanguageDetectorModule = {
  type: "languageDetector",
  detect: () => storageGet(KEY) ?? Intl.DateTimeFormat().resolvedOptions().locale,
  cacheUserLanguage: (lng) => storageSet(KEY, lng),
};

/** A device's app has no document to label. */
export function showLanguage(_lng: string): void {}
