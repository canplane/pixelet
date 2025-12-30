/**
 * [three-config]
 * 렌더링 품질과 성능을 제어하는 설정 파일입니다.
 * 모든 거리 단위는 Meter이며, 단일 변수(VIEW_DISTANCE)를 통해 모든 임계값을 자동으로 계산합니다.
 */

// 🎯 [Control-Knob] 이 값만 바꾸면 모든 렌더링 거리가 자동 조정됩니다.
// 기존 1024 -> 3500 (3.5km)로 대폭 상향
const VIEW_DISTANCE = 6000;

const DEFAULT_TILT: number = 30; // 30도 Tilt

export const THREE_CONFIG = {
  /**
   * [world-layout]
   * 월드의 물리적인 배치와 레이어 순서를 정의합니다.
   */
  LAYOUT: {
    /**
     * [floor-rotation]
     * Three.js 기본 Plane(XY)을 지리 좌표계(XZ)로 변환합니다.
     * 1. Geo X (동서) -> Three X
     * 2. Geo Y (남북) -> Three -Z (북쪽)
     * 3. Geo Z (고도) -> Three Y (위쪽)
     */
    FLOOR_ROTATION: [-Math.PI / 2, 0, 0] as [number, number, number],

    /**
     * [layer-offsets]
     * 동일 평면상의 텍스처 간섭(Z-Fighting)을 방지하기 위한 미세 높이값입니다.
     * 🎯 0.01m(1cm)는 모바일 GPU의 낮은 Depth 정밀도에서도 안정적입니다.
     */
    OFFSET: {
      TERRAIN: 0,
      GRID: 0, // polygonOffset 사용
      HIGHLIGHTER: 0.01,
    },
  },

  /**
   * [camera-physics]
   * 카메라 뷰포트 및 조작 한계를 정의합니다.
   */
  CAMERA: {
    FOV: 50,
    NEAR: 0.1, // 거리가 멀어지면 정밀도 문제(Z-fighting) 방지를 위해 NEAR도 살짝 올리는 게 좋습니다.

    DEFAULT_DISTANCE: 256, // [zoom-start]

    /**
     * [polar-angle]
     * 0(수직 위) ~ PI(수직 아래).
     * DEFAULT_TILT(30도) 적용 시 지면을 60도 각도로 내려다봅니다.
     */
    DEFAULT_POLAR_ANGLE: ((90 - DEFAULT_TILT) * Math.PI) / 180,

    MIN_DISTANCE: 16,

    // 🎯 [derived-limit] 카메라가 데이터 한계선까지 갈 수 있게 허용
    MAX_DISTANCE: VIEW_DISTANCE,
  },

  /**
   * [rendering-effects]
   * 시각적 품질과 성능의 균형을 맞추는 설정입니다.
   */
  RENDERING: {
    // 🎯 [derived-fading] 끝부분 20% 구간에서만 자연스럽게 페이드 아웃
    FADE_START: VIEW_DISTANCE * 0.8,
    FADE_END: VIEW_DISTANCE,

    // 🎯 [gpu-safety] 렌더링 한계는 시야 거리보다 넉넉하게(1.5배) 잡아야 잘림 현상이 없습니다.
    FAR_CLIP: VIEW_DISTANCE * 1.5,

    // [fog-density] 거리가 멀어질수록 안개는 옅어져야 합니다. (반비례 관계)
    // 1000m일 때 0.0008이었다면, 3500m일 때는 약 0.00025 정도가 적당합니다.
    FOG_DENSITY: 0.8 / VIEW_DISTANCE,
  },
} as const;
