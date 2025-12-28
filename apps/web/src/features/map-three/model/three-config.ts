/**
 * [three-config]
 * 이 파일은 외부 라이브러리 의존성이 없는 순수 설정(Configuration) 파일입니다.
 * 모든 거리 단위는 Meter이며, 2의 배수(Binary-friendly) 수치를 사용하여 연산 최적화를 수행합니다.
 */

/**
 * [가시거리 및 데이터 리밋 설계 원칙]
 * 1. DATA_LIMIT (1024m): 실제 OSM 데이터를 메쉬화하여 보여주는 물리적 한계선입니다.
 * 2. MAX_DISTANCE (1024m): 카메라가 원점에서 물러날 수 있는 한계로, 데이터가 없는 허공을 보지 않도록 차단합니다.
 * 3. FADE_END (896m): 데이터의 날카로운 절단면을 숨기기 위해 사물이 완전히 투명해지는 지점입니다.
 * 4. FAR_CLIP (2048m): GPU가 렌더링을 수행하는 절대 한계선으로, 연산 자원 낭비를 방지합니다.
 */

const DATA_LIMIT: number = 1024;
const DEFAULT_TILT: number = 30; // [default-tilt] 지면 기준 각도 (사용자 친화적 수치)

export const THREE_CONFIG = {
  /**
   * [layout]
   * 월드의 물리적인 배치와 레이어 순서를 정의합니다.
   */
  LAYOUT: {
    /**
     * [floor-rotation]
     * Three.js의 기본 Plane은 XY 평면(수직)입니다.
     * 이를 [ -Math.PI / 2, 0, 0 ] 회전시키면:
     * 1. 지리적 X(동서) -> Three.js World X
     * 2. 지리적 Y(남북) -> Three.js World -Z (북쪽) 🎯
     * 3. 지리적 Z(고도) -> Three.js World Y (위) 🎯
     */
    FLOOR_ROTATION: [-Math.PI / 2, 0, 0] as [number, number, number],
    // [layer-offsets] 레이어 간 간섭(Z-Fighting)을 방지하기 위한 미세 높이값들
    OFFSET: {
      TERRAIN: 0,
      GRID: 0, // polygonOffset으로 처리하므로 0
      HIGHLIGHTER: 0.01, // 🚀 [Z-Fighting] 0.001보다는 0.01이 모바일 환경에서 더 안정적입니다.
    },
  },

  /**
   * [camera]
   * 카메라 물리 설정 및 뷰포트 기준을 정의합니다.
   */
  CAMERA: {
    FOV: 50,
    NEAR: 0.1,

    DEFAULT_DISTANCE: 256, // [zoom-start] 시작 시 적절한 줌 레벨
    /**
     * [polar-angle]
     * Three.js에서 0도는 수직 위(+Y)를 봅니다.
     * 90도는 수평(지평선)을 봅니다.
     * DEFAULT_TILT가 30도라면, 카메라는 지면을 향해 60도 각도로 내려다봅니다.
     */
    DEFAULT_POLAR_ANGLE: ((90 - DEFAULT_TILT) * Math.PI) / 180,

    // 🚀 [binary-distance-limit] 2의 배수 기반 거리 제한
    MIN_DISTANCE: 16,
    MAX_DISTANCE: DATA_LIMIT, // [data-limit-sync] 데이터 가용 범위까지만 줌 아웃 허용
  },

  /**
   * [rendering]
   * 렌더링 및 시각 효과(가시거리 제어)를 담당합니다.
   * 최적화: 가시 범위를 제한하여 GPU 부하를 줄이고 지평선을 자연스럽게 처리합니다.
   */
  RENDERING: {
    // 🚀 [cut-plane-masking] 절단면 마스킹: 데이터 끝(1024m) 전부터 페이드 처리
    FADE_START: 512, // [fade-start] 50% 지점부터 서서히 투명해짐
    FADE_END: 896, // [fade-end] 약 87% 지점에서 완전히 사라져 절단면 은닉

    FAR_CLIP: 2048, // [gpu-limit] GPU 렌더링 마지노선 (타일 시스템의 절반 수준)

    // [fog-density] 안개 농도: 값이 커질수록 대기감이 강해지며 데이터 끝부분을 가려줍니다.
    FOG_DENSITY: 0.0008,

    /**
     * [floating-origin-reset]
     * 🔄 Floating Origin 리셋 임계값
     * 정밀도 유지를 위해 원점에서 1280m 이상 멀어지면 세상을 원점으로 다시 당깁니다.
     * 안개/페이드 영역보다 살짝 바깥으로 잡아 시각적 튀는 현상을 방지합니다.
     */
    RESET_THRESHOLD: 1280,
  },
} as const;
