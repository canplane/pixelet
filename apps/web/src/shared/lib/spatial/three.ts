/**
 * @file three.ts
 * [Adapter] Core(Spatial) <-> Three.js (View)
 * * * [Coordinate Systems]
 * - Core(wPos): 3857 (x:East, y:North, z:Ele) -> Z-Up
 * - View(vPos): Three (x:Right, y:Up, z:Back) -> Y-Up
 */

import type { Vector3Tuple } from 'three';
import type { Pos3, XY, XYZ } from '@pixelet/core/shared/lib/spatial/primitives';

export type WPosOptionalZ = XY & { z?: number };

/**
 * [toViewArray]
 * Core 좌표를 Three.js Vector3Tuple로 변환 (Swizzling 적용)
 * wPos.z(Ele) -> vPos.y(Up)
 * wPos.y(North) -> vPos.-z(Back)
 */
export const toViewArray = (wPos: WPosOptionalZ): Vector3Tuple => {
  const { x, y, z = 0 } = wPos;
  return [x, z, -y];
};

/**
 * [toViewArrayWithOffset]
 * 고도(y) 오프셋을 적용하여 변환 (Z-Fighting 방지용)
 */
export const toViewArrayWithOffset = (wPos: WPosOptionalZ, yOffset: number = 0): Vector3Tuple => {
  const { x, y, z = 0 } = wPos;
  return [x, z + yOffset, -y];
};

/**
 * [fromViewArray]
 * Three.js 좌표를 Core Pos3로 복원
 */
export const fromViewArray = (vArr: Vector3Tuple): Pos3 => {
  const [vx, vy, vz] = vArr;
  return {
    x: vx,
    y: -vz,
    z: vy,
  } as Pos3;
};
