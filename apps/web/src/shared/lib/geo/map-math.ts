import { projection } from '@/shared/lib/geo/projection';
import type { Location, MeterPos } from '@/shared/types/map';

const DEG_TO_RAD = Math.PI / 180;

/**
 * 위경도와 고도를 3D 공간의 절대 미터 좌표(MeterPos)로 변환합니다.
 * * - 1 unit = 1 meter (1:1 스케일)
 * - Mercator 왜곡 보정: Web Mercator 투영 시 고위도로 갈수록 비대해지는 선형 왜곡을
 * 현재 위도의 cos(lat)을 곱해 실제 지표면 미터 단위로 압축 보정합니다.
 */
const locationToMeter = ({ lng, alt = 0, lat }: Location): MeterPos => {
  // 1. Web Mercator 표준 투영 (미터 단위 좌표 추출)
  const absX = projection.lngToX(lng);
  const absY = projection.latToY(lat);

  /**
   * 2. 위도에 따른 Mercator 거리 왜곡 보정 (cosLat)
   * 실제 지표면 거리 = Mercator 절대 거리 * cos(latitude)
   */
  const cosLat = Math.cos(lat * DEG_TO_RAD);

  /**
   * 3. Three.js 좌표계 매핑
   * - 위도 상승(+lat) -> WebMercator(+Y) -> Three.js 화면 안쪽(-Z)
   * - 고도(alt) -> Three.js 수직 상승(+Y)
   */
  return {
    x: absX * cosLat,
    y: alt, // 고도는 왜곡 보정에서 제외(수직 거리이므로)
    z: -absY * cosLat,
  };
};

/**
 * 3D 공간의 절대 미터 좌표(MeterPos)를 다시 위경도 및 고도로 역변환합니다.
 * * locationToMeter에서 적용했던 cosLat 압축을 수학적으로 정밀하게 해제합니다.
 */
const meterToLocation = ({ x, y, z }: MeterPos): Location => {
  /**
   * 1. 1차 위도 추정 (Initial Approximation)
   * 현재 z는 cosLat이 곱해져 압축된 상태이므로, projection.yToLat(-z)를
   * 그대로 호출하면 원래보다 적도에 가까운(낮은) 위도가 계산됩니다.
   */
  const initialLat = projection.yToLat(-z);

  /**
   * 2. Mercator 좌표 복원 (Coordinate Restoration)
   * 1차 추정된 위도의 코사인 값을 이용해, 압축되기 전의 원본 Mercator 좌표를 추정합니다.
   * 공식: z = -absY * cosLat  =>  absY = -z / cosLat
   */
  const cosLat = Math.cos(initialLat * DEG_TO_RAD);
  const recoveredAbsY = -z / cosLat;
  const recoveredAbsX = x / cosLat;

  /**
   * 3. 최종 위경도 및 고도 산출
   * 복원된 Mercator 절대 좌표를 사용하여 실제 위경도를 구하고,
   * Y축 값을 고도(alt)로 복구합니다.
   */
  return {
    lng: projection.xToLng(recoveredAbsX),
    lat: projection.yToLat(recoveredAbsY),
    alt: y,
  };
};

export const mapMath = {
  locationToMeter,
  meterToLocation,
};
