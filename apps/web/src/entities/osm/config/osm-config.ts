// 이 파일은 OSM 데이터 통신 및 메모리 관리를 위한 순수 설정 파일입니다.

export const OSM_CONFIG = {
  // --- API 통신 제어 ---
  REQUEST_INTERVAL: 8000, // API 재요청 최소 간격 (8초)
  DISTANCE_THRESHOLD: 300, // 마지막 요청지 기준 이동 임계값 (300m)
  DEFAULT_RADIUS: 1500, // 한 번에 요청할 데이터 반경 (1.5km): FADE_END(1.2km)보다 살짝 크게 잡아 여유분 확보

  // --- 메모리 관리 (GC) ---
  // 중심점에서 이 거리 이상 멀어진 데이터는 성능을 위해 삭제합니다.
  // [중요] THREE_CONFIG.RENDERING.FOG_FAR(1200m)보다 커야 끊김이 없습니다.
  CLEANUP_THRESHOLD: 2500, // 데이터 삭제 기준 거리 (2.5km)

  // --- 엔드포인트 로드 밸런싱 ---
  ENDPOINTS: [
    'https://overpass-api.de/api/interpreter', // 독일 (공식 메인)
    'https://overpass.openstreetmap.fr/api/interpreter', // 프랑스 (안정적)
    'https://overpass.kumi.systems/api/interpreter', // Kumi Systems (고성능)
    'https://maps.mail.ru/osm/tools/overpass/api/interpreter', // 러시아 (아시아권 속도 양호)
    'https://overpass.nchc.org.tw/api/interpreter', // 대만 (한국과 물리적으로 가까움)
  ],
} as const;
