/**
 * i18n initialization
 *
 * This file sets up internationalization using i18next and react-i18next.
 * Translation resources are loaded dynamically from JSON files.
 */

import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';
import LanguageDetector from 'i18next-browser-languagedetector';
import HttpBackend from 'i18next-http-backend';

/**
 * i18next configuration overview
 *
 * - Translation resources are loaded via HTTP (JSON files)
 * - The user's language is detected automatically from the browser
 * - Language preference can be overridden via localStorage or cookies
 *
 * Resource file structure:
 *   public/locales/{language}/{namespace}.json
 *
 * Example:
 *   public/locales/en/common.json
 *   public/locales/ko/common.json
 *
 * To manually test a language in the browser:
 *
 * ```ts
 * localStorage.i18nextLng = 'ko'; // or 'en'
 * location.reload();
 * ```
 */

i18n
  // Load translations from JSON files over HTTP
  .use(HttpBackend)

  // Detect user language from browser / storage
  .use(LanguageDetector)

  // Bind i18next to React
  .use(initReactI18next)

  .init({
    // Default language if detection fails
    fallbackLng: 'en',

    // Disable key nesting (treat keys as plain strings)
    keySeparator: false,

    // React already escapes values, so disable double-escaping
    interpolation: {
      escapeValue: false,
    },

    // Default namespaces
    ns: ['common'],
    fallbackNS: ['common'],

    backend: {
      // Path pattern for translation files
      loadPath: (lng: string, ns: string) => `locales/${lng}/${ns}.json`,
    },

    detection: {
      // Language detection priority
      order: ['cookie', 'localStorage', 'navigator'],

      // Persist detected language
      caches: ['localStorage', 'cookie'],
    },
  });

export default i18n;
