// @pixelet/shared/spec/space/adapter/yup.ts

import type { f64 } from "@pixelet/shared/kit/word.ts";

import { fill3, type V3 } from "@pixelet/shared/spec/space/v3.ts";

/**
 * Legacy Y-up <-> Z-up conversion helpers.
 *
 * Current web world/camera runtime is native Z-up.
 * This module remains only for:
 * - legacy Three.js Y-up assets/primitives,
 * - compatibility transforms,
 * - localized interop at Y-up boundaries.
 *
 * Basis transform:
 *   Z-up (X, Y, Z) -> Y-up Three-style (X, Z, -Y)
 */

/** Legacy rotation for mounting Y-up XY-plane content into the Z-up world. */
export const YUP_TO_ZUP_ROTATION = [-Math.PI / 2, 0, 0] as const;

/** Converts a Z-up vector into legacy Y-up Three-style coordinates. */
export function yup(zup: Readonly<V3>, out: V3 = fill3(0)): V3 {
  const zup_x: f64 = zup[0];
  const zup_y: f64 = zup[1];
  const zup_z: f64 = zup[2];
  out[0] = zup_x;
  out[1] = zup_z;
  out[2] = -zup_y;
  return out;
}

/** Converts a legacy Y-up vector back into the native Z-up world basis. */
export function zup(yup: Readonly<V3>, out: V3 = fill3(0)): V3 {
  const yup_x: f64 = yup[0];
  const yup_y: f64 = yup[1];
  const yup_z: f64 = yup[2];
  out[0] = yup_x;
  out[1] = -yup_z;
  out[2] = yup_y;
  return out;
}
