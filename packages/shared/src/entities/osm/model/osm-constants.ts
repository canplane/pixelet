/**
 * [osm-constants]
 * OSM 데이터의 기하학적 구조와 격자 시스템을 정의하는 순수 상수 파일입니다.
 * 이 수치들은 클라이언트와 워커, 서버 간의 데이터 호환성을 보장하는 핵심 규격입니다.
 */

const TILE_SIZE = 4096;
const CELL_SIZE = 2;

/** 계산된 타일당 해상도 (픽셀 수) */
const TILE_RESOLUTION = Math.floor(TILE_SIZE / CELL_SIZE);

export const OSM_CONSTANTS = {
  /**
   * [fixed-grid-strategy]
   * 1. TILE_SIZE (4096m): 지구 좌표를 4km 단위 격자로 나누어 관리합니다.
   * 2. DEFAULT_RENDER_RADIUS (1): 플레이어가 속한 타일을 중심으로 주변 1칸(3x3, 총 9타일)의 데이터를 유지합니다.
   */
  TILE_SIZE,
  DEFAULT_RENDER_RADIUS: 1,
  CLEANUP_RADIUS: 2, // 로딩보다 1단위 더 넓게 설정

  /**
   * [baking-resolution]
   * CELL_SIZE: 비트맵 생성 시 1픽셀이 담당할 실제 거리(m)입니다.
   * 중앙 참조를 통해 타일 베이커와 프로세서의 해상도를 동기화합니다.
   */
  CELL_SIZE,

  // 타일 가로/세로당 복셀 셀 개수 (예: 4096 / 2 = 2048)
  TILE_RESOLUTION,

  /**
   * [z-scale-ratio]
   * 수평 격자(cell-size) 대비 수직 격자의 배율을 결정합니다.
   *
   * * Formula: voxelZUnit = CELL_SIZE * Z_SCALE_RATIO
   * * Example: CELL_SIZE(2m) * RATIO(2) = 4m (수직 1층당 높이)
   *
   * [purpose]
   * - 시각적 강조: 지형의 고도 변화를 수직으로 과장하여 입체감을 높임.
   * - 데이터 범위: Uint8(0~255) 한계 내에서 더 높은 해발 고도까지 표현 가능.
   */
  Z_SCALE_RATIO: 2,

  // 🎯 [optimization] 클라이언트와 동일한 128m 간격 샘플링 (33x33)
  // 타일 당 쿼리 수를 4백만 개에서 약 1,000개로 줄여 DB 부하를 제거합니다.
  Z_SAMPLING_STEP: 128,

  /**
   * [TERRAIN_SEGMENTS]
   * 지형 메쉬를 구성하는 분할 수입니다. (4096 / 128 = 32)
   * 🎯 [Engineering Fix] Math.round를 사용하여 부동소수점 오차(ex: 31.99)에 의한
   * 세그먼트 유실을 방지하고, 데이터 샘플 밀도와 메쉬 정점 수(Segments + 1)를 1:1로 강제합니다.
   */
  get TERRAIN_SEGMENTS() {
    return Math.round(this.TILE_SIZE / this.Z_SAMPLING_STEP);
  },

  /**
   * [floating-origin-policy]
   * Floating Origin 리셋을 위한 타일 버퍼 크기입니다.
   *
   * 🎯 [hysteresis-buffer]
   * 현재 타일에서 '2칸(8km)' 이상 벗어나면 리셋을 트리거합니다.
   * 1칸 차이는 경계에서 빈번하게 발생할 수 있으므로, 2칸을 두어 불필요한 연산을 방지합니다.
   */
  TILE_BUFFER: 2,
} as const;
