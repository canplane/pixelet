// @pxl/web/locale/i18n.ts

import i18next from "i18next";
import { initReactI18next } from "react-i18next";
import LanguageDetector from "i18next-browser-languagedetector";

import en from "./en.json" with { type: "json" };
import ko from "./ko.json" with { type: "json" };
import enDev from "./dev/en.json" with { type: "json" };
import koDev from "./dev/ko.json" with { type: "json" };

/**
 * Copy for development-only surfaces lives in its own catalogue, folded in
 * only when __IS_DEV__ is true. Gating the *components* is not enough: their
 * strings would still ride along inside the bundled catalogue, so a
 * production build shipped the debug section's labels — every toggle it
 * offers, named — even with the section itself eliminated. Behind a
 * compile-time constant the merge and both dev imports drop out entirely.
 */
interface Catalogue {
  [key: string]: string | Catalogue;
}

/**
 * Merges every level, not just the top one — a shallow merge silently drops
 * a nested dev-only branch instead of erroring, so a gated component would
 * render its raw key instead of its label.
 */
function deepMerge(base: Catalogue, extra: Catalogue): Catalogue {
  const out: Catalogue = { ...base };
  for (const [key, value] of Object.entries(extra)) {
    const current = out[key];
    out[key] = typeof value === "object" && typeof current === "object" ? deepMerge(current, value) : value;
  }
  return out;
}

function withDev<T>(base: T, dev: unknown): T {
  if (!__IS_DEV__) return base;
  return deepMerge(base as Catalogue, dev as Catalogue) as T;
}

let booted = false;

/**
 * Initializes i18next for the app. Called from bootstrapApp() rather
 * than running as a side effect of importing a module, so that every
 * one-time global setup happens in one place, in a defined order, inside
 * main.tsx's error handling — a side-effect import runs at module-eval time,
 * which is outside that try/catch and ordered by import hoisting rather than
 * by intent.
 *
 * Catalogues are bundled rather than fetched, which makes this synchronous:
 * `init()` has its resources in hand and `t()` is correct from the first
 * render. Fetching them meant every string rendered before the request landed
 * showed its raw key — visible on a cold load as a flash of `settings.title`
 * and friends, and unfixable from the call sites, since the gap is in
 * initialization rather than in any one component.
 *
 * The cost is 10.5 kB of JSON in a 1.97 MB bundle, and re-deploying to change
 * a string — accepted because the app ships as a built bundle anyway, so a
 * copy change is always a deploy regardless of where the catalogue lives.
 */
export function bootstrapI18n(): void {
  if (booted) return;
  booted = true;

  i18next
    .use(LanguageDetector)
    .use(initReactI18next)
    .init({
      fallbackLng: "en",
      supportedLngs: ["en", "ko"],
      ns: ["translation"],
      defaultNS: "translation",
      resources: {
        en: { translation: withDev(en, enDev) },
        ko: { translation: withDev(ko, koDev) },
      },
      detection: {
        order: ["localStorage", "navigator"],
        caches: ["localStorage"],
        lookupLocalStorage: "pxl:lang",
      },
      interpolation: {
        // React already escapes interpolated values when rendering — i18next's
        // own escaping on top of that would double-escape entities like &.
        escapeValue: false,
      },
      react: {
        useSuspense: false,
      },
    });

  // index.html ships a static lang="en" for crawlers, which never run the
  // bundle that would otherwise correct it. For real visitors, keep it in
  // sync with whatever i18next actually resolves/switches to — detection
  // is synchronous (see above), so this is correct from the first paint.
  document.documentElement.lang = i18next.language;
  i18next.on("languageChanged", (lng) => {
    document.documentElement.lang = lng;
  });
}
