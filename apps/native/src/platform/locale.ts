// @pxl/native/platform/locale.ts

import type { LanguageDetectorModule } from "i18next";

import { storage, STORAGE_KEYS } from "@/kernel/abi/host.spec.ts";
import { storageGet, storageSet } from "./storage.ts";

const KEY = STORAGE_KEYS[storage.LANG];

export const languageDetector: LanguageDetectorModule = {
  type: "languageDetector",
  detect: () => storageGet(KEY) ?? Intl.DateTimeFormat().resolvedOptions().locale,
  cacheUserLanguage: (lng) => storageSet(KEY, lng),
};

export function showLanguage(_lng: string): void {}
