// @pxl/client/shell/locale/i18n.ts

import i18next, { type LanguageDetectorModule } from "i18next";
import { initReactI18next } from "react-i18next";

import en from "./en.json" with { type: "json" };
import ja from "./ja.json" with { type: "json" };
import ko from "./ko.json" with { type: "json" };
import zhHans from "./zh-Hans.json" with { type: "json" };
import zhHant from "./zh-Hant.json" with { type: "json" };
import enDev from "./dev/en.json" with { type: "json" };
import jaDev from "./dev/ja.json" with { type: "json" };
import koDev from "./dev/ko.json" with { type: "json" };
import zhHansDev from "./dev/zh-Hans.json" with { type: "json" };
import zhHantDev from "./dev/zh-Hant.json" with { type: "json" };
import * as locale from "@/service/locale.ts";
import * as storage from "@/service/storage.ts";

interface Catalog {
  [key: string]: string | Catalog;
}

/** Deep, since a shallow merge silently drops nested dev-only branches. */
function deepMerge(base: Catalog, extra: Catalog): Catalog {
  const out: Catalog = { ...base };
  for (const [key, value] of Object.entries(extra)) {
    const current = out[key];
    out[key] =
      typeof value === "object" && typeof current === "object"
        ? deepMerge(current, value)
        : value;
  }
  return out;
}

/** Dev catalogs only behind a constant at top level: Rolldown does not drop a
 * helper's argument. */
const catalog = __IS_DEV__
  ? {
      en: deepMerge(en, enDev) as typeof en,
      ko: deepMerge(ko, koDev) as typeof ko,
      ja: deepMerge(ja, jaDev) as typeof ja,
      "zh-Hans": deepMerge(zhHans, zhHansDev) as typeof zhHans,
      "zh-Hant": deepMerge(zhHant, zhHantDev) as typeof zhHant,
    }
  : { en, ko, ja, "zh-Hans": zhHans, "zh-Hant": zhHant };

export type Language = keyof typeof catalog;

/** Each language by its own name. */
export const LANGUAGE_NAMES: Record<Language, string> = {
  en: "English",
  ko: "한국어",
  ja: "日本語",
  "zh-Hans": "简体中文",
  "zh-Hant": "繁體中文",
};

const LANGUAGES = Object.keys(catalog) as Language[];

const LANG_KEY = "pxl:lang";

/** The catalog a system's tag reads: its language, and for Chinese its script,
 * Taiwan's, Hong Kong's and Macau's being Traditional where the tag names none. */
function languageOf(tag: string): string {
  const [language] = tag.split("-");
  if (language !== "zh") return language;
  return /^zh-(Hant|TW|HK|MO)/.test(tag) ? "zh-Hant" : "zh-Hans";
}

/** The language chosen, else the first supported of the system's preferences, else en. */
const languageDetector: LanguageDetectorModule = {
  type: "languageDetector",
  // By language alone: i18next takes a later exact match over an earlier
  // region, so ["en-US", "ko"] read as Korean.
  detect: () => storage.get(LANG_KEY) ?? locale.list().map(languageOf),
  cacheUserLanguage: (lng) => storage.set(LANG_KEY, lng),
};

let booted = false;

/**
 * Called by the app before the first render (web main.tsx, native index.ts). Catalogs are
 * bundled, so `t()` is correct from the first render.
 */
export function boot(): void {
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
      resources: Object.fromEntries(
        LANGUAGES.map((lng) => [lng, { translation: catalog[lng] }]),
      ),
      interpolation: {
        // React already escapes; i18next's would double-escape.
        escapeValue: false,
      },
      react: {
        useSuspense: false,
      },
    });

  locale.show(i18next.language);
  i18next.on("languageChanged", (lng) => locale.show(lng));
}
