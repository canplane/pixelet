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
   * [zScaleRatio 설명]
   * 수평 격자(cellSize) 대비 수직 격자의 배율을 결정합니다.
   * * 예시: cellSize = 2m, zScaleRatio = 4 인 경우
   * 1. voxelZUnit = 2m * 4 = 8m
   * 2. 실제 고도가 8m 상승할 때마다 복셀 데이터(Uint8) 상의 1층이 증가합니다.
   * 3. 결과적으로 복셀 한 칸의 규격은 [2m(W) x 2m(D) x 8m(H)] 인 직육면체가 됩니다.
   * * * [주요 목적]
   * - 시각적 강조: 지형의 고도 변화를 수직으로 과장하여 입체감을 높임.
   * - 데이터 범위: Uint8(0~255) 한계 내에서 더 높은 해발 고도까지 표현 가능.
   */
  Z_SCALE_RATIO: 2,

  /**
   * --- API 모드 설정 ---
   * 'local': Docker로 띄운 로컬 서버 사용 (속도 최상, 제한 없음)
   * 'public': 공용 서버 목록 사용 (Legacy, 로드밸런싱 및 Rate Limit 적용)
   */
  API_MODE: 'public' as 'local' | 'public', // 여기서 모드 변경

  /**
   * --- 로컬 서버 엔드포인트 ---
   * Docker 컨테이너가 실행 중인 주소입니다.
   */
  LOCAL_API_ENDPOINT: '/api/geo/tile',

  /**
   * --- 엔드포인트 (Load Balancing Pool) ---
   * 한국에서의 접속 속도 및 서버 안정성을 고려하여 목록을 10개로 확장했습니다.
   * * interpreter 주소가 /api/ 인지 /cgi/ 인지 주의해야 합니다.
   */
  PUBLIC_API_ENDPOINTS: [
    // 1. 아시아/러시아권 (한국에서 비교적 빠름)
    'https://overpass.nchc.org.tw/api/interpreter', // 대만 (1순위 권장)
    'https://maps.mail.ru/osm/tools/overpass/api/interpreter', // 러시아 (Mail.ru)
    'https://overpass.openstreetmap.ru/cgi/interpreter', // 러시아 (OSM Community)

    // 2. 유럽권 (고성능 서버)
    'https://overpass.osm.ch/api/interpreter', // 스위스 (매우 빠르고 안정적)
    'https://overpass.kumi.systems/api/interpreter', // 유럽 (고성능, 슬롯 제한 엄격)
    'https://api.openstreetmap.fr/oapi/interpreter', // 프랑스 (OpenStreetMap France)
    'https://overpass.openstreetmap.fr/api/interpreter', // 프랑스 (백업)

    // 3. 메인 서버군 (독일) - 트래픽이 많지만 용량이 큼
    'https://overpass-api.de/api/interpreter', // 독일 메인
    'https://lz4.overpass-api.de/api/interpreter', // 독일 (LZ4 압축, 응답 빠름)
    'https://z.overpass-api.de/api/interpreter', // 독일 (Z 압축)
  ],

  /**
   * --- API 통신 제어 ---
   * REQUEST_TIMEOUT: Overpass API 응답 대기 시간(ms)입니다.
   * 타일 시스템 도입으로 DISTANCE_THRESHOLD는 더 이상 사용하지 않고 타일 경계 이동을 기준으로 요청합니다.
   */
  REQUEST_TIMEOUT: 30000,
} as const;

/** 계산된 타일당 해상도 (픽셀 수) */
export const TILE_RESOLUTION = Math.floor(
  OSM_CONFIG.TILE_SIZE / OSM_CONFIG.CELL_SIZE,
);
