import type { Location, MeterPos } from '@/shared/types/map';

/**
 * [projection]
 * 지리적 좌표(WGS84)와 3D 월드 좌표(Web Mercator) 간의 변환을 담당하는 엔진의 핵심 유틸리티입니다.
 */

/**
 * 메르카토르 도법(Web Mercator EPSG:3857) 수학적 한계치
 * 위도가 극점에 가까워지면 Y값이 무하게 발산하므로 렌더링 가능한 임계치를 정의합니다.
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
const EARTH_RADIUS = 6378137;

export const projection = {
  EARTH_RADIUS,

  /**
   * [to-local]
   * 절대 미터 좌표를 현재 씬의 원점(offset) 기준 상대 좌표로 변환합니다.
   */
  toLocal: (abs: MeterPos, offset: MeterPos): [number, number, number] => [
    abs.x - offset.x,
    abs.y - offset.y,
    abs.z - offset.z,
  ],

  /**
   * [clamp]
   * 입력된 위경도 및 고도 좌표가 투영 가능한 수학적 범위를 넘지 않도록 보정합니다.
   */
  clamp: (location: Location): Location => {
    const { lng, alt, lat } = location;

    return {
      lng: Math.max(LIMITS.LNG_MIN, Math.min(LIMITS.LNG_MAX, lng)),
      lat: Math.max(LIMITS.LAT_MIN, Math.min(LIMITS.LAT_MAX, lat)),
      ...(alt !== undefined && {
        alt: Math.max(LIMITS.ALT_MIN, Math.min(LIMITS.ALT_MAX, alt)),
      }),
    };
  },

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
   * 위경도(Location)를 절대 미터 좌표(MeterPos)로 투영합니다.
   */
  project: (location: Location) => {
    const { lng, lat, alt = 0 } = projection.clamp(location);
    return {
      x: projection.lngToX(lng),
      y: alt,
      z: -projection.latToY(lat),
    };
  },

  /**
   * [unproject]
   * 절대 미터 좌표를 위경도 좌표로 역투영합니다.
   */
  unproject: (pos: MeterPos) => {
    return {
      lng: projection.xToLng(pos.x),
      alt: pos.y,
      lat: projection.yToLat(-pos.z),
    };
  },

  /**
   * [create-bbox]
   * 중심점과 미터 범위를 기반으로 외부 API(Overpass 등)용 BBox 문자열을 생성합니다.
   * 반환 형식: "south,west,north,east"
   */
  createBBox: (
    centerLng: number,
    centerLat: number,
    widthMeter: number,
    heightMeter: number = widthMeter,
  ): string => {
    const centerX = projection.lngToX(centerLng);
    const centerY = projection.latToY(centerLat);

    const halfW = widthMeter / 2;
    const halfH = heightMeter / 2;

    const minX = centerX - halfW;
    const maxX = centerX + halfW;
    const minY = centerY - halfH;
    const maxY = centerY + halfH;

    const south = projection.yToLat(minY);
    const west = projection.xToLng(minX);
    const north = projection.yToLat(maxY);
    const east = projection.xToLng(maxX);

    return `${south},${west},${north},${east}`;
  },
} as const;
