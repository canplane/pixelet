// 이 파일은 지리적 좌표(WGS84)와 3D 월드 좌표(Web Mercator) 간의 변환을 담당하는 물리 엔진의 핵심입니다.

import type { Location } from '@/shared/types/map';

/**
 * 메르카토르 도법(Web Mercator EPSG:3857) 수학적 한계치
 * 위도가 약 85.05도 이상으로 커지면 Y값이 무한대로 발산하여 가드가 필요합니다.
 */
const LIMITS = {
  LAT_MAX: 85.05112878,
  LAT_MIN: -85.05112878,
  LNG_MAX: 180,
  LNG_MIN: -180,
  ALT_MAX: 10000,
  ALT_MIN: -500,
} as const;

/**
 * WGS84 타원체를 구체로 가정했을 때의 지구 반지름 (단위: m)
 * Web Mercator 투영법에서 사용하는 표준 값입니다.
 */
const EARTH_RADIUS = 6378137;

/**
 * 시스템 전반에서 사용하는 격자 및 투영 설정
 */
export const projection = {
  EARTH_RADIUS,

  /**
   * 격자 한 칸의 물리적 크기 (단위: 미터)
   * 1:1 시스템이므로 4m 격자는 엔진 내 4 Unit 크기를 가집니다.
   */
  GRID_SIZE: 4,

  /**
   * [Guard] 입력된 좌표가 한계를 넘지 않도록 보정 (alt가 없으면 객체에서 제거)
   */
  clamp: (location: Location): Location => {
    const { lng, alt, lat } = location;

    return {
      lng: Math.max(LIMITS.LNG_MIN, Math.min(LIMITS.LNG_MAX, lng)),
      lat: Math.max(LIMITS.LAT_MIN, Math.min(LIMITS.LAT_MAX, lat)),
      // alt가 존재할 때만 객체에 alt 키를 포함시킴
      ...(alt !== undefined && {
        alt: Math.max(LIMITS.ALT_MIN, Math.min(LIMITS.ALT_MAX, alt)),
      }),
    };
  },

  /**
   * 경도(Longitude) -> Web Mercator X 좌표 (미터)
   */
  lngToX: (lng: number): number => {
    // 수학적 오류 방지를 위한 경도 가드 적용
    const clampedLng = Math.max(LIMITS.LNG_MIN, Math.min(LIMITS.LNG_MAX, lng));
    return ((clampedLng * Math.PI) / 180) * EARTH_RADIUS;
  },

  /**
   * 위도(Latitude) -> Web Mercator Y 좌표 (미터)
   */
  latToY: (lat: number): number => {
    // 위도가 90도에 가까워지면 tan 값이 무한대로 발산하므로 반드시 가드 필요
    const clampedLat = Math.max(LIMITS.LAT_MIN, Math.min(LIMITS.LAT_MAX, lat));
    const latRad = (clampedLat * Math.PI) / 180;
    return Math.log(Math.tan(Math.PI / 4 + latRad / 2)) * EARTH_RADIUS;
  },

  /**
   * Web Mercator X 좌표 -> 경도(Longitude)
   */
  xToLng: (x: number): number => (x / EARTH_RADIUS) * (180 / Math.PI),

  /**
   * Web Mercator Y 좌표 -> 위도(Latitude)
   */
  yToLat: (y: number): number => {
    const latRad = 2 * Math.atan(Math.exp(y / EARTH_RADIUS)) - Math.PI / 2;
    return latRad * (180 / Math.PI);
  },
} as const;
