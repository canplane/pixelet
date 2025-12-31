/**
 * @file app-config.ts
 * 애플리케이션의 전역 실행 환경 및 서비스 모드를 정의합니다.
 */

/** 🎯 1. 모드 정의 (상수/타입) */

/**
 * [ServiceMode]
 * 시스템 전체의 데이터 처리 및 통신 전략을 결정합니다.
 */
export const SERVICE_MODES = {
  INTERNAL: 'internal',
  EXTERNAL: 'external',
} as const;

export type ServiceMode = (typeof SERVICE_MODES)[keyof typeof SERVICE_MODES];

/** 🌍 2. 실제 런타임 값 (환경 변수 읽기) */

// APP_CONFIG 객체 내부로 숨기지 않고, 서비스의 핵심 상태이므로 직접 export 합니다.
export const SERVICE_MODE: ServiceMode =
  (import.meta.env.VITE_SERVICE_MODE as ServiceMode) || SERVICE_MODES.EXTERNAL;

// /** ⚙️ 3. 전역 설정 객체 */

// export const APP_CONFIG = {
//   // 서비스 모드에 따른 파생 설정이나 기타 상수들을 관리
//   API_TIMEOUT: 10000,
//   MAX_RETRIES: 3,
// } as const;

// /** 💡 4. 비즈니스 로직 헬퍼 (권장) */
// // "이게 external인가?" 라고 묻는 것보다 "제한적인 상황인가?"라고 묻는 것이
// // 하부 도메인(Baker, Assembler) 로직과 결합도가 낮아집니다.

// /** 헬퍼: 현재 리소스 제한 모드 여부 */
// export const isLimitedMode = CURRENT_SERVICE_MODE === SERVICE_MODES.EXTERNAL;
