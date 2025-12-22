// 이 파일은 OSM 데이터 통신 및 메모리 관리를 위한 순수 설정 파일입니다.

export const OSM_CONFIG = {
  // --- API 통신 제어 ---
  REQUEST_INTERVAL: 10000, // API 재요청 최소 간격 (ms)
  DISTANCE_THRESHOLD: 400, // 마지막 요청지 기준 이동 임계값 (m)

  // 🚀 핵심: 반경을 줄여야 합니다.
  // 1.5km 반경은 면적으로 치면 엄청납니다. 안개(1.5km)를 살짝 포기하더라도
  // 모바일에선 1.0km ~ 1.2km가 한계치입니다.
  DEFAULT_RADIUS: 1000, // 한 번에 요청할 데이터 반경 (m)

  // --- 메모리 관리 (GC) ---
  // 🚀 모바일은 메모리가 적으므로 2.5km까지 들고 있을 여유가 없습니다.
  // 시야에서 사라지면(1.5km) 거의 즉시(1.8km) 지워야 합니다.
  CLEANUP_THRESHOLD: 1500,

  // --- 엔드포인트 로드 밸런싱 ---
  ENDPOINTS: [
    'https://overpass-api.de/api/interpreter', // 독일 (공식 메인)
    'https://overpass.openstreetmap.fr/api/interpreter', // 프랑스 (안정적)
    'https://overpass.kumi.systems/api/interpreter', // Kumi Systems (고성능)
    'https://maps.mail.ru/osm/tools/overpass/api/interpreter', // 러시아 (아시아권 속도 양호)
    'https://overpass.nchc.org.tw/api/interpreter', // 대만 (한국과 물리적으로 가까움)
  ],
} as const;
