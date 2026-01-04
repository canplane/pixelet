/**
 * i18n initialization (Client)
 *
 * 브라우저 환경(Web)에서 플러그인과 설정을 병합하여 초기화합니다.
 */

import { initReactI18next } from 'react-i18next';
import LanguageDetector from 'i18next-browser-languagedetector';
// import HttpBackend from 'i18next-http-backend';
import i18n, { i18nConfig } from './i18n.core';

i18n
  // .use(HttpBackend) // Load translations from JSON files over HTTP
  .use(LanguageDetector) // Detect user language from browser / storage
  .use(initReactI18next) // Bind i18next to React
  .init({
    ...i18nConfig, // 공통 설정 상속
    // backend: {
    //   loadPath: (lng: string, ns: string) => `locales/${lng}/${ns}.json`, // Path pattern for translation files
    // },
    detection: {
      order: ['cookie', 'localStorage', 'navigator'], // Language detection priority
      caches: ['localStorage', 'cookie'], // Persist detected language
    },
  });

export default i18n;
