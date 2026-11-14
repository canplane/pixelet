// @pxl/client/ui/shared/locale/i18n.ts

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
import * as locale from "@/service/platform/locale.ts";
import * as storage from "@/service/platform/storage.ts";

interface Catalog {
  [key: string]: string | Catalog;
}

/** A shallow merge would let a dev branch replace the base's branch of the
 * same name and lose the base's keys under it. */
function deep_merge(base: Catalog, extra: Catalog): Catalog {
  const out: Catalog = { ...base };
  for (const [key, value] of Object.entries(extra)) {
    const current = out[key];
    out[key] =
      typeof value === "object" && typeof current === "object"
        ? deep_merge(current, value)
        : value;
  }
  return out;
}

/** The dev catalogs are merged behind `__DEV__` tested at the top level, so
 * that a release bundle drops them. Tree shaking does not drop an argument
 * passed to a helper that tests the flag. */
const catalog = __DEV__
  ? {
      en: deep_merge(en, enDev) as typeof en,
      ko: deep_merge(ko, koDev) as typeof ko,
      ja: deep_merge(ja, jaDev) as typeof ja,
      "zh-Hans": deep_merge(zhHans, zhHansDev) as typeof zhHans,
      "zh-Hant": deep_merge(zhHant, zhHantDev) as typeof zhHant,
    }
  : { en, ko, ja, "zh-Hans": zhHans, "zh-Hant": zhHant };

export type Language = keyof typeof catalog;

export const LANGUAGE_NAMES: Record<Language, string> = {
  en: "English",
  ko: "한국어",
  ja: "日本語",
  "zh-Hans": "简体中文",
  "zh-Hant": "繁體中文",
};

const LANGUAGES = Object.keys(catalog) as Language[];

const LANG_KEY = "pxl:lang";

/** A Chinese tag that names no script reads Traditional for Taiwan, Hong Kong
 * and Macau, and Simplified elsewhere. */
function language_of(tag: string): string {
  const [language] = tag.split("-");
  if (language !== "zh") return language;
  return /^zh-(Hant|TW|HK|MO)/.test(tag) ? "zh-Hant" : "zh-Hans";
}

/** The person's chosen language comes first, then the first of the system's
 * preferred languages that has a catalog, then English. */
const language_detector: LanguageDetectorModule = {
  type: "languageDetector",
  // The system's tags are cut to their languages. Given whole tags, i18next
  // prefers a later exact match to an earlier tag's region, so ["en-US", "ko"]
  // would read as Korean.
  detect: () => storage.get(LANG_KEY) ?? locale.list().map(language_of),
  cacheUserLanguage: (lng) => storage.set(LANG_KEY, lng),
};

let booted = false;

/**
 * The app calls this before its first render (ts/client/index.ts). The
 * catalogs are bundled, so `t()` gives the right words from the first render.
 */
export function boot(): void {
  if (booted) return;
  booted = true;

  i18next
    .use(language_detector)
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
        // React escapes text itself, so i18next's escaping would escape it
        // twice.
        escapeValue: false,
      },
      react: {
        useSuspense: false,
      },
    });

  locale.show(i18next.language);
  i18next.on("languageChanged", (lng) => locale.show(lng));
}
