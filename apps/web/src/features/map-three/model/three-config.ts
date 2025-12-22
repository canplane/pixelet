// 이 파일은 외부 라이브러리 의존성이 없는 순수 설정(Configuration) 파일입니다.

const DATA_LIMIT = 1000; // OSM_CONFIG.DEFAULT_RADIUS와 동기화

const DEFAULT_TILT = 30; // 지면 기준 각도 (사용자 친화적 수치)

export const THREE_CONFIG = {
  /**
   * 카메라 물리 설정 및 뷰포트 기준 (Three.js 내장 속성 위주)
   */
  CAMERA: {
    FOV: 50,
    NEAR: 0.1,

    DEFAULT_DISTANCE: 200,
    DEFAULT_POLAR_ANGLE: ((90 - DEFAULT_TILT) * Math.PI) / 180,
    // DEFAULT_TILT, // UI나 다른 계산에서 참조용

    // 🚀 계산식 기반 거리 제한
    MIN_DISTANCE: 10,
    MAX_DISTANCE: DATA_LIMIT, // 데이터가 있는 곳까지만 줌 아웃 허용
  },

  /**
   * 렌더링 및 시각 효과 (가시거리 제어)
   * 최적화 포인트: 가시거리를 1km~1.5km 내외로 좁혀 렌더링 부하를 줄입니다.
   */
  RENDERING: {
    // 🚀 데이터 끝(1000m) 전부터 페이드 시작
    FADE_START: DATA_LIMIT * 0.6, // 서서히 투명해짐
    FADE_END: DATA_LIMIT * 0.9, // 완전히 사라짐

    // 🚀 안개는 데이터가 사라지기 전에 화면을 가려야 함
    FOG_NEAR: DATA_LIMIT * 0.5, // 안개가 더 일찍 시작되어 멀리 있는 끊김을 방어 (너무 일찍 뿌얘지면 답답할 수 있음)
    FOG_FAR: DATA_LIMIT * 0.9, // UserMarker 가시성과 동기화
    FAR_CLIP: 2000, // 카메라가 그리는 최대 한계선

    /**
     * 🔄 Floating Origin 리셋 임계값
     * 안개가 완전히 끝나는 지점(FOG_FAR)보다 조금 더 여유를 둡니다.
     * 이 지점에 도달하면 세상을 원점으로 당겨 정밀도를 회복합니다.
     */
    RESET_THRESHOLD: DATA_LIMIT * 0.9 * 1.25,
  },
} as const; // 값을 고정하여 타입 추론 최적화
