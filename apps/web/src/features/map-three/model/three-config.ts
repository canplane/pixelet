// 이 파일은 외부 라이브러리 의존성이 없는 순수 설정(Configuration) 파일입니다.

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
    DEFAULT_TILT, // UI나 다른 계산에서 참조용
  },

  /**
   * 렌더링 및 시각 효과 (가시거리 제어)
   * 최적화 포인트: 가시거리를 1km~1.5km 내외로 좁혀 렌더링 부하를 줄입니다.
   */
  RENDERING: {
    FADE_START: 800, // 800m부터 서서히 투명해짐
    FADE_END: 1200, // 1.2km 지점에서 완전히 사라짐 (643개 -> 약 150~200개 예상)
    FOG_NEAR: 600, // 600: 안개가 더 일찍 시작되어 멀리 있는 끊김을 방어
    FOG_FAR: 1200, // 데이터 소멸 지점과 안개 끝 지점을 맞춤
    FAR_CLIP: 3000, // 카메라가 그리는 최대 한계선 (3km)
  },
} as const; // 값을 고정하여 타입 추론 최적화
