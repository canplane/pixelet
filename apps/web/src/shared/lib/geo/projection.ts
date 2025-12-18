/**
 * 지리적/수학적 상수 및 변환 함수 (Web Mercator)
 */

/**
 * WGS84 타원체를 구체로 가정했을 때의 지구 반지름 (단위: m)
 * Web Mercator(EPSG:3857) 투영법에서 사용하는 표준 값입니다.
 * (위경도를 미터 단위 좌표로 변환할 때 기준이 됩니다.)
 */
export const EARTH_RADIUS = 6378137;

export const projection = {
  EARTH_RADIUS,

  /**
   * Three.js 세계와 현실 미터법의 비율
   * 4일 경우: 1 unit = 4 meter
   * (좌표 계산 및 mapConverter에서 기준으로 사용)
   */
  SCALE_FACTOR: 1,

  // 경도 -> X 좌표 (미터)
  lngToX: (lng: number): number => ((lng * Math.PI) / 180) * EARTH_RADIUS,

  // 위도 -> Y 좌표 (미터)
  latToY: (lat: number): number => {
    const latRad = (lat * Math.PI) / 180;
    return Math.log(Math.tan(Math.PI / 4 + latRad / 2)) * EARTH_RADIUS;
  },

  // X 좌표 -> 경도
  xToLng: (x: number): number => (x / EARTH_RADIUS) * (180 / Math.PI),

  // Y 좌표 -> 위도
  yToLat: (y: number): number => {
    const latRad = 2 * Math.atan(Math.exp(y / EARTH_RADIUS)) - Math.PI / 2;
    return latRad * (180 / Math.PI);
  },
};
