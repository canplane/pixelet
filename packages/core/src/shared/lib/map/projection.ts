import type { Coord4326, Coord } from '@pixelet/core/shared/model/coords';

/**
 * [projection]
 * 지리적 좌표(WGS84, EPSG:4326)와 표준 투영 좌표(Web Mercator, EPSG:3857) 간의
 * 순수한 변환을 담당하는 엔진의 핵심 데이터 레이어 유틸리티입니다.
 * 렌더링을 위한 축 교정은 이 레이어가 아닌 렌더링 레이어에서 처리합니다.
 */

/**
 * 메르카토르 도법(Web Mercator EPSG:3857) 수학적 한계치
 * 위도가 극점에 가까워지면 Y값이 무한하게 발산하므로 투영 가능한 임계치를 정의합니다.
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
 */
const EARTH_RADIUS = 6378137; // EPSG:3857 기준 지구 반지름 (m)
const DEG_TO_RAD = Math.PI / 180;

export const projection = {
  EARTH_RADIUS,

  /**
   * [to-local]
   * 절대 미터 좌표를 특정 기준점(offset) 기반의 상대 미터 좌표로 변환합니다.
   * 순수 지리적 축(X:동, Y:북, Z:고도)을 유지합니다.
   */
  toLocal: (abs: Coord, offset: Coord): Coord => ({
    x: abs.x - offset.x,
    y: abs.y - offset.y,
    z: abs.z - offset.z,
  }),

  /**
   * [lng-to-x]
   * 경도(Longitude)를 Web Mercator X 좌표(미터)로 변환합니다.
   */
  lngToX: (lng: number): number => {
    const clampedLng = Math.max(LIMITS.LNG_MIN, Math.min(LIMITS.LNG_MAX, lng));
    return ((clampedLng * Math.PI) / 180) * EARTH_RADIUS;
  },

  /**
   * [lat-to-y]
   * 위도(Latitude)를 Web Mercator Y 좌표(미터)로 변환합니다.
   */
  latToY: (lat: number): number => {
    const clampedLat = Math.max(LIMITS.LAT_MIN, Math.min(LIMITS.LAT_MAX, lat));
    const latRad = (clampedLat * Math.PI) / 180;
    return Math.log(Math.tan(Math.PI / 4 + latRad / 2)) * EARTH_RADIUS;
  },

  /**
   * [x-to-lng]
   * Web Mercator X 좌표를 경도(Longitude)로 역변환합니다.
   */
  xToLng: (x: number): number => (x / EARTH_RADIUS) * (180 / Math.PI),

  /**
   * [y-to-lat]
   * Web Mercator Y 좌표를 위도(Latitude)로 역변환합니다.
   */
  yToLat: (y: number): number => {
    const latRad = 2 * Math.atan(Math.exp(y / EARTH_RADIUS)) - Math.PI / 2;
    return latRad * (180 / Math.PI);
  },

  // --- [좌표 변환 유틸리티] ---

  /**
   * [project]
   * 위경도(Coord4326)를 EPSG:3857 표준 미터 좌표(Coord)로 투영합니다.
   * 결과값은 PostGIS의 ST_Transform(geom, 3857) 결과와 완벽히 일치합니다.
   * * 주의: 데이터 정합성을 위해 표준 Web Mercator(EPSG:3857) 수치를 그대로 반환합니다.
   */
  project: (location: Coord4326): Coord => {
    const { lng, lat, alt = 0 } = projection.clamp(location);
    let x = projection.lngToX(lng);
    let y = projection.latToY(lat);

    return {
      x,
      y, // 지리적 북쪽 방향 (Standard 3857 Y)
      z: alt, // 지리적 고도 방향
    };
  },

  /**
   * [unproject]
   * EPSG:3857 절대 미터 좌표를 위경도 좌표(Coord4326)로 역투영합니다.
   */
  unproject: (pos: Coord): Coord4326 => {
    return {
      lng: projection.xToLng(pos.x),
      lat: projection.yToLat(pos.y),
      alt: pos.z,
    };
  },

  /**
   * [haversine-distance]
   * 하버사인 공식을 사용하여 두 위경도 좌표 사이의 대권 거리(Great-circle distance)를 계산합니다.
   * @param start 시작 지점 위경도 (Coord4326)
   * @param end 종료 지점 위경도 (Coord4326)
   * @returns 실제 지표면 기준 미터(m) 단위 거리
   */
  getDistance: (start: Coord4326, end: Coord4326): number => {
    const dLat = (end.lat - start.lat) * DEG_TO_RAD;
    const dLon = (end.lng - start.lng) * DEG_TO_RAD;

    const a =
      Math.sin(dLat / 2) * Math.sin(dLat / 2) +
      Math.cos(start.lat * DEG_TO_RAD) *
        Math.cos(end.lat * DEG_TO_RAD) *
        Math.sin(dLon / 2) *
        Math.sin(dLon / 2);

    const c = 2 * Math.atan2(Math.sqrt(a), Math.sqrt(1 - a));
    return EARTH_RADIUS * c;
  },

  /**
   * [clamp]
   * 입력된 위경도 및 고도 좌표가 투영 가능한 수학적 범위를 넘지 않도록 보정합니다.
   */
  clamp: (location: Coord4326): Coord4326 => {
    const { lng, alt, lat } = location;

    return {
      lng: Math.max(LIMITS.LNG_MIN, Math.min(LIMITS.LNG_MAX, lng)),
      lat: Math.max(LIMITS.LAT_MIN, Math.min(LIMITS.LAT_MAX, lat)),
      ...(alt !== undefined && {
        alt: Math.max(LIMITS.ALT_MIN, Math.min(LIMITS.ALT_MAX, alt)),
      }),
    };
  },
} as const;
