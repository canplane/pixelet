/**
 * @file osm-constants.ts
 * OSM 데이터의 지오메트리 규격, 격자 시스템 및 메모리 정책을 정의하는 핵심 상수입니다.
 * 이 파일의 수치는 DB 쿼리(Server), 데이터 가공(Worker), 렌더링(Client) 전체의 동기화를 보장합니다.
 */

const TILE_SIZE = 1024; // 타일 한 변의 물리적 길이 (meters)
const CELL_SIZE = 2; // 최소 데이터 단위(Voxel/Pixel)의 물리적 크기 (meters)

/** * [TILE_RES] 타일 해상도
 * 한 타일의 가로/세로에 포함되는 셀(Cell)의 개수입니다.
 * 비트맵 버퍼 크기 및 인덱싱의 기준이 됩니다.
 */
const TILE_RES = Math.floor(TILE_SIZE / CELL_SIZE);

export const OSM_CONSTANTS = {
  /**
   * [Fixed Grid Strategy]
   * 지구 좌표계를 1024m(약 1km) 단위의 정방형 격자로 분할하여 관리합니다.
   */
  TILE_SIZE,

  /**
   * [Baking Resolution]
   * 데이터 처리 및 텍스처 베이킹 시 1픽셀이 담당하는 실제 거리입니다.
   * 값이 작을수록 정밀도가 높아지나 메모리 사용량이 지수적으로 증가합니다.
   */
  CELL_SIZE,

  /**
   * [Z_UNIT_SCALE] 수직 해상도 및 왜곡 배율
   * 데이터 1유닛(Voxel Layer)이 물리적으로 담당하는 수직 거리(m)를 결정합니다.
   * * 1. 해상도 제어: 이 값이 높을수록 수직 데이터를 더 미세하게 양자화(Quantization)합니다.
   * 2. 의도적 왜곡: 렌더링 시 1유닛은 CELL_SIZE 높이로 투영되므로, 수직 강조 효과를 조절합니다.
   * * @formula 1 Unit Data = CELL_SIZE / Z_UNIT_SCALE (m)
   */
  Z_UNIT_SCALE: 1,

  /** 타일당 복셀 해상도 (예: 1024 / 1 = 1024px) */
  TILE_RES,

  /**
   * [LOAD_RADIUS] 데이터 로드 반경
   * 플레이어 중심 타일로부터 데이터를 미리 가져오는(Fetch) 반경입니다.
   * 값 2: 5x5 영역(총 25개 타일)을 워커에 요청하여 메모리에 상주시키며,
   * 실제 화면에 그리는 RENDER_RADIUS보다 최소 1단계 이상 커야 Pop-in 현상을 방지합니다.
   */
  LOAD_RADIUS: 2,

  /**
   * [CLEANUP_RADIUS] 메모리 해제 반경
   * LOAD_RADIUS보다 크게 설정하여 경계 지역에서 빈번한 로드/언로드를 방지(Hysteresis)합니다.
   * 이 범위를 벗어난 타일은 메모리(TileStore) 및 GPU 리소스에서 즉시 제거됩니다.
   */
  CLEANUP_RADIUS: 4,

  /**
   * [ELEVATION_SAMPLE_STEP] 지형 고도 샘플링 간격
   * PostGIS 래스터 쿼리 및 외부 Elevation API 호출 시 지표면 데이터를 추출하는 간격(m)입니다.
   * * 🎯 [Strict Alignment]
   * 이 값은 반드시 TILE_SIZE의 약수여야 타일 간 경계(Seam)에서 높이 오차가 발생하지 않습니다.
   * 예: 64m 간격 시 1024m 타일 한 변당 17개의 포인트(Padding 제외)가 생성됩니다.
   */
  ELEVATION_SAMPLE_STEP: 64,

  /**
   * [REBASE_FLOATING_ORIGIN_THRES] 월드 앵커 재설정 임계값 (단위: Tile)
   * Floating Origin 시스템에서 좌표 정밀도를 유지하기 위해 월드 기준점(Anchor)을 다시 찍는 기준입니다.
   * * 🎯 [Logic]
   * 현재 Anchor 타일로부터 이 거리 이상 멀어지면, 현재 타일을 새로운 Anchor로 설정(Rebase)합니다.
   * 이를 통해 카메라와 객체 간의 거리 차이를 줄여 GPU 연산의 수치적 안정성을 확보합니다.
   */
  REBASE_FLOATING_ORIGIN_THRES: 8,
} as const;
