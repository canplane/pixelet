// @pxl/shared/spec/space/adapter/yup.ts

import type { f64 } from "@xpute/core/kit/word.ts";

import { fill3, type V3 } from "@pxl/shared/spec/space/v3.ts";

/**
 * Y-up <-> Z-up conversion, for boundaries where Three.js's default basis
 * meets this project's.
 *
 * **Nothing imports this today, and that is the expected state, not rot.**
 * The engine is natively Z-up: `bootstrap/three.ts` sets
 * `THREE.Object3D.DEFAULT_UP` to (0, 0, 1) globally, and the 2026-04 Z-up
 * unification deleted the per-subsystem Y-up adaptations that used to call
 * helpers like these. Inside the engine there is no longer anywhere for a
 * conversion to happen — ENU maps straight onto the render axes.
 *
 * What it is kept for has not arrived yet: **imported assets.** All geometry
 * is currently generated in code (BufferGeometry / InstancedMesh built by
 * chunk_stage, location_puck, and friends), and the repository
 * has no GLTF/GLB loader at all. The moment one is added — avatars or props
 * for the social layer, say — it lands Y-up, because Blender and the glTF
 * spec both are, and this is the seam that converts it.
 *
 * So: don't reach for these while working inside the world; the engine
 * basis is Z-up end to end. Reach for them only at an asset or third-party
 * boundary, and keep the conversion localized there rather than letting
 * Y-up coordinates travel inward.
 *
 * Basis transform:
 *   Z-up (X, Y, Z) -> Y-up Three-style (X, Z, -Y)
 */

/** Rotation for mounting Y-up XY-plane content into the Z-up world. */
export const YUP_TO_ZUP_ROTATION = [-Math.PI / 2, 0, 0] as const;

/** Converts a Z-up vector into Y-up Three-style coordinates. */
export function yup(zup: Readonly<V3>, out: V3 = fill3(0)): V3 {
  const zup_x: f64 = zup[0];
  const zup_y: f64 = zup[1];
  const zup_z: f64 = zup[2];
  out[0] = zup_x;
  out[1] = zup_z;
  out[2] = -zup_y;
  return out;
}

/** Converts a Y-up vector back into the native Z-up world basis. */
export function zup(yup: Readonly<V3>, out: V3 = fill3(0)): V3 {
  const yup_x: f64 = yup[0];
  const yup_y: f64 = yup[1];
  const yup_z: f64 = yup[2];
  out[0] = yup_x;
  out[1] = -yup_z;
  out[2] = yup_y;
  return out;
}
