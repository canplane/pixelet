/**
 * OSM 데이터 요청 및 동기화 설정
 */

export const OSM_CONFIG = {
  REQUEST_INTERVAL: 10000, // API 재요청 최소 간격 (10초)
  DISTANCE_THRESHOLD: 150, // 마지막 요청지 기준 이동 임계값 (150m)
  DEFAULT_RADIUS: 1000, // 한 번에 요청할 데이터 반경 (1km)

  // 데이터 파기 기준 (데이터 본연의 규칙)
  CLEANUP_THRESHOLD: 5500, // 메모리 관리를 위해 데이터를 삭제하는 거리 (5.5km)

  // 가용 가능한 엔드포인트 목록 (클라이언트 사이드 로드 밸런싱)
  ENDPOINTS: [
    'https://overpass-api.de/api/interpreter', // 독일 (공식 메인)
    'https://overpass.openstreetmap.fr/api/interpreter', // 프랑스 (안정적)
    'https://overpass.kumi.systems/api/interpreter', // Kumi Systems (고성능)
    'https://maps.mail.ru/osm/tools/overpass/api/interpreter', // 러시아 (아시아권 속도 양호)
    'https://overpass.nchc.org.tw/api/interpreter', // 대만 (한국과 물리적으로 가까움)
  ],
};
