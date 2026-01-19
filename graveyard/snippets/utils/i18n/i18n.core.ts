// // @pxl/common/lib/i18n/i18n.core.ts

// /**
//  * i18n initialization
//  *
//  * 여기서는 인스턴스만 생성하고, 공통 설정값(Options)만 변수로 관리합니다.
//  * 실제 .init()은 여기서 하지 않습니다.
//  */

// import i18n, { type i18n as I18nInstance, type InitOptions } from "i18next";
// // import koCommon from "./locales/ko/common.json" with { type: "json" };
// // import enCommon from "./locales/en/common.json" with { type: "json" };

// // 공통 설정 정의
// export const i18nConfig: InitOptions = {
//   resources: {
//     // ko: { common: koCommon },
//     // en: { common: enCommon },
//   },
//   fallbackLng: "en", // Default language if detection fails
//   keySeparator: false, // Disable key nesting (treat keys as plain strings)
//   // React already escapes values, so disable double-escaping
//   interpolation: {
//     escapeValue: false,
//   },
//   // Namespaces
//   ns: ["common"],
//   defaultNS: "common",
// };

// // 인스턴스만 생성
// const i18nInstance: I18nInstance = i18n.createInstance();

// export default i18nInstance;
