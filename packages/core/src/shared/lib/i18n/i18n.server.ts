/**
 * i18n initialization (Server)
 *
 * Node 환경(API)에서 사용할 엔트리입니다. 여기서 최종 초기화를 수행합니다.
 */

import i18n, { i18nConfig } from './i18n.core';

// 서버 환경에 맞는 초기화 실행
i18n.init(i18nConfig);

export default i18n;
