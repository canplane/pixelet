/**
 * @file three-coords.ts
 * @description 표준 Coord(3857)와 렌더링 ThreeCoord(Three.js) 간의 좌표 변환
 */

import type { Vector3Tuple } from 'three';
import type { Coord } from '@pixelet/shared/shared/model/coords';

/**
 * [OptionalZ]
 * Coord에서 x, y는 필수지만 z는 선택사항인 타입을 정의합니다.
 */
type CoordOptionalZ = Omit<Coord, 'z'> & { z?: number };

/** * [to-three-coord]
 * Coord(지리 표준)를 Three.js 렌더링용 배열로 변환합니다.
 * 🎯 지리 z(고도) -> Three y
 * 🎯 지리 y(북쪽) -> Three -z
 */
export const toThreeCoord = (pos: CoordOptionalZ): Vector3Tuple => {
  const { x, y, z = 0 } = pos; // z가 없으면 0으로 간주
  return [x, z, -y];
};

/**
 * [to-three-coord-with-offset]
 * 변환과 동시에 고도 오프셋을 적용하는 단축 함수입니다.
 */
export const toThreeCoordWithOffset = (
  pos: CoordOptionalZ,
  zOffset: number = 0,
): Vector3Tuple => {
  const { x, y, z = 0 } = pos; // z가 없으면 0으로 간주
  return [x, z + zOffset, -y];
};

/** * [from-three-coord]
 * Three.js 좌표를 다시 지리 표준 Coord 객체로 복원합니다.
 */
export const fromThreeCoord = (tPos: Vector3Tuple): Coord => {
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
export const withZOffset = (pos: Coord, offset: number): Coord => ({
  ...pos,
  z: pos.z + offset,
});
