import type { Vector3Tuple } from 'three';
import type { MeterPos } from '@pixelet/shared/shared/model/coords';

/**
 * [OptionalZ]
 * MeterPos에서 x, y는 필수지만 z는 선택사항인 타입을 정의합니다.
 */
type MeterPosOptionalZ = Omit<MeterPos, 'z'> & { z?: number };

/** * [to-three-pos]
 * MeterPos(지리 표준)를 Three.js 렌더링용 배열로 변환합니다.
 * 🎯 지리 z(고도) -> Three y
 * 🎯 지리 y(북쪽) -> Three -z
 */
export const toThreePos = (pos: MeterPosOptionalZ): Vector3Tuple => {
  const { x, y, z = 0 } = pos; // z가 없으면 0으로 간주
  return [x, z, -y];
};

/**
 * [to-three-pos-with-offset]
 * 변환과 동시에 고도 오프셋을 적용하는 단축 함수입니다.
 */
export const toThreePosWithOffset = (
  pos: MeterPosOptionalZ,
  zOffset: number = 0,
): Vector3Tuple => {
  const { x, y, z = 0 } = pos; // z가 없으면 0으로 간주
  return [x, z + zOffset, -y];
};

/** * [from-three-pos]
 * Three.js 좌표를 다시 지리 표준 MeterPos 객체로 복원합니다.
 */
export const fromThreePos = (tPos: Vector3Tuple): MeterPos => {
  const [tx, ty, tz] = tPos;
  return {
    x: tx,
    y: -tz, // -z 방향이 북쪽(+y)
    z: ty, // 위쪽(+y)이 고도(z)
  };
};

/**
 * [with-z-offset]
 * 기존 지리 좌표의 고도(z)값만 미세하게 조정합니다.
 * 레이어 간의 Z-Fighting 방지나, 물체를 지면에서 살짝 띄울 때 사용합니다.
 */
export const withZOffset = (pos: MeterPos, offset: number): MeterPos => ({
  ...pos,
  z: pos.z + offset,
});
