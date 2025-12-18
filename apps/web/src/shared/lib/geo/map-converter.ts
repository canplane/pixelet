import { projection } from './projection';

/**
 * [Utility] mapConverter
 * 현실 세계의 물리량(m, km)과 3D 월드의 수치(Unit) 사이의 통역사 역할을 합니다.
 * 모든 실제 지표 데이터(건물 크기, 이동 속도, 가시 거리)는 이 컨버터를 거쳐야
 * SCALE_FACTOR 변경 시 앱이 깨지지 않습니다.
 */
export const mapConverter = {
  /**
   * 1. 실제 거리(m) -> 3D 월드 유닛 변환
   * @usage 건물 높이 설정, 이동 속도 계산, 가시 반경 설정 시
   * @example const meshHeight = mapConverter.metersToUnits(50); // 실제 50m 건물
   */
  metersToUnits: (meters: number) => meters / projection.SCALE_FACTOR,

  /**
   * 2. 3D 월드 유닛 -> 실제 거리(m) 변환
   * @usage 두 물체 사이의 유닛 거리를 측정하여 실제 미터로 화면에 표시할 때
   * @example const realDist = mapConverter.unitsToMeters(meshA.position.distanceTo(meshB.position));
   */
  unitsToMeters: (units: number) => units * projection.SCALE_FACTOR,

  /**
   * 3. [Helper] 1 유닛당 실제 미터 비율 반환
   * @usage 물리 엔진 설정이나 커스텀 쉐이더 계산 시 기준값으로 사용
   */
  getMetersPerUnit: () => projection.SCALE_FACTOR,
};
