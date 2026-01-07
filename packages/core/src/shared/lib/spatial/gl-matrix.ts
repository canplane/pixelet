/**
 * @file gl-matrix.ts
 * [Adapter] Spatial Primitives <-> gl-matrix (Array)
 * 배열 기반의 수학 연산을 위한 변환 헬퍼입니다.
 */
import type { Pos2, Pos3, XY, XYZ } from './primitives';

// --- To Array ---
export const toArray2 = (v: XY): [number, number] => [v.x, v.y];
export const toArray3 = (v: XYZ): [number, number, number] => [v.x, v.y, v.z];

// --- From Array ---
export const fromArray2 = (arr: ArrayLike<number>): Pos2 =>
  ({
    x: arr[0],
    y: arr[1],
  }) as Pos2;

export const fromArray3 = (arr: ArrayLike<number>): Pos3 =>
  ({
    x: arr[0],
    y: arr[1],
    z: arr[2],
  }) as Pos3;
