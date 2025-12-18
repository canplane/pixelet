import { projection } from '@/shared/lib/geo/projection';
import type {
  LngLat,
  AbsoluteWorldPos,
  RelativeWorldPos,
} from '@/shared/types/map';

const DEG_TO_RAD = Math.PI / 180;

/**
 * 1단계: 위경도를 전 지구 표준 미터 좌표로 변환 (Web Mercator)
 *
 * 위경도를 전 지구 공통 단위인 '미터(m)'로 변환합니다. (Web Mercator EPSG:3857)
 * * [중요] 왜곡 알림:
 * 이 단계의 값은 '지도상의 미터'입니다. 실제 지구는 구체이므로
 * 평면 지도(Mercator)로 펼치는 과정에서 고위도로 갈수록 실제보다 거리가 더 길게 측정되는
 * '선형 왜곡'이 이미 포함되어 있습니다.
 */

const lngLatToAbs = ([lng, lat]: LngLat): AbsoluteWorldPos => {
  const absX = projection.lngToX(lng);
  const absY = projection.latToY(lat);
  // Three.js: 위도 상승(+Y)은 화면 안쪽(-Z) 방향으로 매핑
  return [absX, 0, -absY];
};

const absToLngLat = ([absX, , absZ]: AbsoluteWorldPos): LngLat => {
  const lng = projection.xToLng(absX);
  const lat = projection.yToLat(-absZ);
  return [lng, lat];
};

/**
 * 2단계: 절대 좌표를 기준점(Origin) 중심의 상대 좌표로 변환
 *
 * 거대한 지구 좌표를 '원점(Origin)' 기준의 작은 숫자로 변환하며, 1단계의 왜곡을 보정합니다.
 * * - Floating Origin: 정밀도 유지를 위해 내 위치(Origin)를 0,0,0으로 설정
 * - CosLat 보정: 1단계(Absolute)에서 발생한 위도별 거리 왜곡을 cos(lat)을 곱해
 * 실제 지표면의 미터(m)와 1:1이 되도록 압축 보정합니다.
 * - Scale Factor: 보정된 실제 미터를 3D 엔진의 몇 유닛으로 표현할지 최종 결정
 */

/**
 * @param targetAbs 변환할 대상의 절대 좌표
 * @param origin 기준점 위경도
 * @param scale 3D 엔진 유닛 비율 (1 unit = scale meters)
 */
const absToRel = (
  targetAbs: AbsoluteWorldPos,
  origin: LngLat,
  scale: number,
): RelativeWorldPos | null => {
  const [originLng, originLat] = origin;
  const [originAbsX, originAbsY, originAbsZ] = lngLatToAbs(origin);
  const [targetAbsX, targetAbsY, targetAbsZ] = targetAbs;

  /**
   * 위도에 따른 Mercator 거리 왜곡 보정 계수
   * 실제 거리 = Mercator 절대 거리 * cos(latitude)
   */
  const cosLat = Math.cos(originLat * DEG_TO_RAD);

  return [
    ((targetAbsX - originAbsX) * cosLat) / scale,
    (targetAbsY - originAbsY) / scale,
    ((targetAbsZ - originAbsZ) * cosLat) / scale,
  ];
};

const relToAbs = (
  relPos: RelativeWorldPos,
  origin: LngLat,
  scale: number,
): AbsoluteWorldPos | null => {
  const [originLng, originLat] = origin;
  const [originAbsX, originAbsY, originAbsZ] = lngLatToAbs(origin);
  const [relX, relY, relZ] = relPos;

  /**
   * 위도에 따른 Mercator 거리 왜곡 보정 계수
   * 실제 거리 = Mercator 절대 거리 * cos(latitude)
   */
  const cosLat = Math.cos(originLat * DEG_TO_RAD);

  return [
    (relX * scale) / cosLat + originAbsX,
    relY * scale + originAbsY,
    (relZ * scale) / cosLat + originAbsZ,
  ];
};

// 순수 함수들로 구성
export const mapMath = {
  lngLatToAbs,
  absToLngLat,
  absToRel,
  relToAbs,
};
