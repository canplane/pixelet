// @pxl/web/bootstrap/i18n.ts

import i18next from "i18next";
import { initReactI18next } from "react-i18next";
import LanguageDetector from "i18next-browser-languagedetector";
import HttpBackend from "i18next-http-backend";

let booted = false;

/**
 * Initializes i18next for the app. Sits alongside bootstrap_three() rather
 * than running as a side effect of importing a module, so that every
 * one-time global setup happens in one place, in a defined order, inside
 * main.tsx's error handling — a side-effect import runs at module-eval time,
 * which is outside that try/catch and ordered by import hoisting rather than
 * by intent.
 *
 * Deliberately not awaited. `init()` kicks off the backend fetch and returns
 * immediately; rendering proceeds while translations are still in flight, and
 * `useSuspense: false` below means a `t()` call that lands first just renders
 * the key for that one pass. Awaiting would block first paint on a network
 * round trip instead — a different tradeoff than the one this app has chosen.
 */
export function bootstrap_i18n(): void {
  if (booted) return;
  booted = true;

  // Locale JSON lives in public/locales/{{lng}}/translation.json (Vite serves
  // public/ at the site root, so this resolves to /locales/ko/translation.json
  // etc. in both dev and prod) — fetched at runtime rather than bundled, so a
  // copy fix doesn't need a rebuild.
  i18next
    .use(HttpBackend)
    .use(LanguageDetector)
    .use(initReactI18next)
    .init({
      fallbackLng: "ko",
      supportedLngs: ["ko", "en"],
      ns: ["translation"],
      defaultNS: "translation",
      backend: {
        loadPath: "/locales/{{lng}}/{{ns}}.json",
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
}
