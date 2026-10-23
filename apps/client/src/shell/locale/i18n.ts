// @pxl/client/shell/locale/i18n.ts

import i18next, { type LanguageDetectorModule } from "i18next";
import { initReactI18next } from "react-i18next";

import { preferredLanguages, showLanguage } from "@/bracket.tsx";
import { storageGet, storageSet } from "@/bracket.tsx";

import en from "./en.json" with { type: "json" };
import ko from "./ko.json" with { type: "json" };
import enDev from "./dev/en.json" with { type: "json" };
import koDev from "./dev/ko.json" with { type: "json" };

interface Catalog {
  [key: string]: string | Catalog;
}

/** Deep, since a shallow merge silently drops nested dev-only branches. */
function deepMerge(base: Catalog, extra: Catalog): Catalog {
  const out: Catalog = { ...base };
  for (const [key, value] of Object.entries(extra)) {
    const current = out[key];
    out[key] = typeof value === "object" && typeof current === "object" ? deepMerge(current, value) : value;
  }
  return out;
}

/** Dev catalogs only behind a constant at top level, not via a helper:
 * Rolldown did not drop a helper's argument and shipped 47 dev strings. */
const catalog = __IS_DEV__ ? { en: deepMerge(en, enDev) as typeof en, ko: deepMerge(ko, koDev) as typeof ko } : { en, ko };

export type Language = keyof typeof catalog;

/** Each language by its own name. */
export const LANGUAGE_NAMES: Record<Language, string> = { en: "English", ko: "한국어" };

const LANGUAGES = Object.keys(catalog) as Language[];

const LANG_KEY = "pxl:lang";

/** The language chosen, else the first supported of the system's preferences, else en. */
const languageDetector: LanguageDetectorModule = {
  type: "languageDetector",
  // By language alone: i18next takes a later exact match over an earlier
  // region, so ["en-US", "ko"] read as Korean.
  detect: () => storageGet(LANG_KEY) ?? preferredLanguages().map((code) => new Intl.Locale(code).language),
  cacheUserLanguage: (lng) => storageSet(LANG_KEY, lng),
};

let booted = false;

/**
 * Called before the first render (main.tsx, bootstrapApp). Catalogs are
 * bundled, so `t()` is correct from the first render; fetching them flashed
 * raw keys on a cold load.
 */
export function bootstrapI18n(): void {
  if (booted) return;
  booted = true;

  i18next
    .use(languageDetector)
    .use(initReactI18next)
    .init({
      fallbackLng: "en",
      supportedLngs: LANGUAGES,
      ns: ["translation"],
      defaultNS: "translation",
      resources: Object.fromEntries(LANGUAGES.map((lng) => [lng, { translation: catalog[lng] }])),
      interpolation: {
        // React already escapes; i18next's would double-escape.
        escapeValue: false,
      },
      react: {
        useSuspense: false,
      },
    });

  showLanguage(i18next.language);
  i18next.on("languageChanged", showLanguage);
}
