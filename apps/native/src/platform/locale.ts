// @pxl/native/platform/locale.ts

import type { LanguageDetectorModule } from "i18next";

import { storageGet, storageSet } from "./storage.ts";

/** The page detector's key. */
const KEY = "pxl:lang";

export const languageDetector: LanguageDetectorModule = {
  type: "languageDetector",
  detect: () => storageGet(KEY) ?? Intl.DateTimeFormat().resolvedOptions().locale,
  cacheUserLanguage: (lng) => storageSet(KEY, lng),
};

export function showLanguage(_lng: string): void {}
