/**
 * [osm-config]
 * OSM 데이터 통신 및 API 인프라 설정을 관리하는 클라이언트 전역 설정입니다.
 */

/**
 * [ServiceMode]
 * 시스템 전체의 데이터 처리 및 통신 전략을 결정합니다.
 */
export type ServiceMode = 'internal' | 'external';

export const OSM_CONFIG = {
  /**
   * [api-mode-config]
   * 환경 변수(VITE_OSM_API_MODE)를 통해 제어합니다.
   * 값이 없으면 기본값으로 'local'을 사용합니다.
   */
  SERVICE_MODE:
    (import.meta.env.VITE_SERVICE_MODE as ServiceMode) || 'external',

  /**
   * [local-endpoint]
   * 환경 변수(VITE_INTERNAL_OSM_API_ENDPOINT)를 통해 제어합니다.
   */
  INTERNAL_OSM_ENDPOINT:
    import.meta.env.VITE_INTERNAL_OSM_API_ENDPOINT || '/api/geo/tile',

  /**
   * [load-balancing-pool]
   * 한국에서의 접속 속도 및 서버 안정성을 고려하여 선별된 공용 서버 목록입니다.
   */
  EXTERNAL_OSM_ENDPOINTS: [
    // 1. 아시아/러시아권 (Latency 우수)
    'https://overpass.nchc.org.tw/api/interpreter', // 대만 (Primary)
    'https://maps.mail.ru/osm/tools/overpass/api/interpreter', // 러시아
    'https://overpass.openstreetmap.ru/cgi/interpreter',

    // 2. 유럽권 (High Performance)
    'https://overpass.osm.ch/api/interpreter', // 스위스
    'https://overpass.kumi.systems/api/interpreter',
    'https://api.openstreetmap.fr/oapi/interpreter', // 프랑스

    // 3. 메인 서버군 (High Capacity, Germany)
    'https://overpass-api.de/api/interpreter',
    'https://lz4.overpass-api.de/api/interpreter', // LZ4 지원
    'https://z.overpass-api.de/api/interpreter',
  ],

  /**
   * [api-request-control]
   * REQUEST_TIMEOUT: Overpass API 응답 대기 시간(ms)
   */
  REQUEST_TIMEOUT: 30000,
} as const;
