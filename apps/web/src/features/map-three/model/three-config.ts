export const THREE_CONFIG = {
  /**
   * 카메라 물리 설정 및 뷰포트 기준 (Three.js 내장 속성 위주)
   */
  CAMERA: {
    FOV: 50,
    NEAR: 0.1,
    DEFAULT_TILT: 30,

    // [추가] 축척 기준: 초기 화면 구성 시 "얼마나 가까이 볼 것인가"를 정의
    // "화면의 REFERENCE_PIXELS 만큼의 너비가 실제 세계의 TARGET_METERS로 보이게 설정한다"는 로직의 근거가 됩니다.
    REFERENCE_PIXELS: 150,
    TARGET_METERS: 50, // 초기 카메라가 지면을 바라보는 기준 거리
  },

  /**
   * 렌더링 및 시각 효과 (가시거리 제어)
   */
  RENDERING: {
    FADE_START: 3000, // 건물이 투명해지기 시작하는 거리 (3km)
    FADE_END: 5000, // 건물이 완전히 사라지는 거리 (5km)
    FOG_NEAR: 2000, // 안개가 끼기 시작하는 지점 (2km)

    // 안개가 화면을 완전히 가리는 지점 (5km)
    // 데이터가 삭제되기 직전에 안개로 가려야 자연스러움 (OSM_CONFIG.CLEANUP_THRESHOLD = 5500)
    FOG_FAR: 5000,

    FAR_CLIP: 10000, // 카메라가 그리는 최대 한계선 (10km)
  },
};
