/**
 * [osm-config]
 * OSM 데이터 통신 및 메모리 관리를 위한 순수 설정 파일입니다.
 * 4096m 고정 격자(Tile) 시스템을 통해 데이터 무결성과 메모리 효율을 보장합니다.
 */
export const OSM_CONFIG = {
  /**
   * --- 타일 기반 제어 (Fixed Grid Strategy) ---
   * 1. TILE_SIZE (4096m): 지구 좌표를 4km 단위 격자로 나누어 관리합니다.
   * 2. DEFAULT_RENDER_RADIUS (1): 플레이어가 속한 타일을 중심으로 주변 1칸(3x3, 총 9타일)의 데이터를 유지합니다.
   */
  TILE_SIZE: 4096,
  DEFAULT_RENDER_RADIUS: 1,

  /**
   * --- 베이킹 해상도 제어 ---
   * CELL_SIZE: 비트맵 생성 시 1픽셀이 담당할 실제 거리(m)입니다.
   * 중앙 참조를 통해 타일 베이커와 프로세서의 해상도를 동기화합니다.
   */
  CELL_SIZE: 2,

  /**
   * --- API 통신 제어 ---
   * REQUEST_TIMEOUT: Overpass API 응답 대기 시간(ms)입니다.
   * 타일 시스템 도입으로 DISTANCE_THRESHOLD는 더 이상 사용하지 않고 타일 경계 이동을 기준으로 요청합니다.
   */
  REQUEST_TIMEOUT: 30000,

  /**
   * --- 엔드포인트 로드 밸런싱 ---
   * 한국 내 서비스 속도를 최적화하기 위해 물리적으로 가까운 대만(NCHC) 서버를 1순위로 배치합니다.
   */
  ENDPOINTS: [
    'https://overpass.nchc.org.tw/api/interpreter', // 대만 (아시아권 최적 속도)
    'https://overpass-api.de/api/interpreter', // 독일 (공식 메인 서버)
    'https://overpass.openstreetmap.fr/api/interpreter', // 프랑스 (안정적 백업)
    'https://overpass.kumi.systems/api/interpreter', // Kumi Systems (고성능)
    'https://maps.mail.ru/osm/tools/overpass/api/interpreter', // 러시아 (아시아권 응답 양호)
  ],
} as const;

/** 계산된 타일당 해상도 (픽셀 수) */
export const TILE_RESOLUTION = Math.floor(
  OSM_CONFIG.TILE_SIZE / OSM_CONFIG.CELL_SIZE,
);
