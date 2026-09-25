// @pxl/native/platform/locale.ts

import type { LanguageDetectorModule } from "i18next";

/** Which language the app opens in: the device's own. A pick is not kept
 * across launches yet — a device's app has no storage wired to it. */
export const languageDetector: LanguageDetectorModule = {
  type: "languageDetector",
  detect: () => Intl.DateTimeFormat().resolvedOptions().locale,
  cacheUserLanguage: () => {},
};

/** A device's app has no document to label. */
export function showLanguage(_lng: string): void {}
