// @pxl/shared/spec/space/xy.ts

import type { f64, i64 } from "@xpute/core/kit/word.ts";

import { type IVec, ivec } from "./pvf3/mod.ts";

export const TO_XY = (i: IVec) => ivec.gather(ivec.X(i), ivec.Y(i), 0n);

// temp
export const WITH_X = (x: i64, i: IVec = 0n): IVec => ivec.gather(BigInt(x), ivec.Y(i), ivec.Z(i));
export const WITH_Y = (y: i64, i: IVec = 0n): IVec => ivec.gather(ivec.X(i), BigInt(y), ivec.Z(i));
export const WITH_Z = (z: i64, i: IVec = 0n): IVec => ivec.gather(ivec.X(i), ivec.Y(i), BigInt(z));

export const CELL_CENTER_U = (u_per_cell: f64): f64 => u_per_cell * 0.5;
// 또는 const CELL_CENTER = U_PER_CELL * 0.5;
