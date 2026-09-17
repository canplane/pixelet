// @pxl/shared/spec/space/adapter/yup.ts

import type { f64 } from "@xpute/core/kit/word.ts";

import { fill3, type V3 } from "@pxl/shared/spec/space/v3.ts";

/**
 * Y-up <-> Z-up conversion on the CPU, for a boundary where a Y-up basis
 * meets this project's.
 *
 * **Nothing imports this, and that is the expected state, not rot.** The
 * engine is Z-up throughout; ENU maps straight onto the render axes. The one
 * Y-up input it has, glTF models, is not converted here either:
 * crates/pxl/core/src/render/gltf/gltf.rs keeps a model's own basis, and
 * render/shader/place.wgsl turns it to Z-up on the GPU as the model is placed.
 *
 * So: don't reach for these while working inside the world. Reach for them
 * only where Y-up coordinates have to be read on the CPU at an asset or
 * third-party boundary, and keep the conversion localized there rather than
 * letting Y-up coordinates travel inward.
 *
 * Basis transform:
 *   Z-up (X, Y, Z) -> Y-up (X, Z, -Y)
 */

/** Rotation for mounting Y-up XY-plane content into the Z-up world. */
export const YUP_TO_ZUP_ROTATION = [-Math.PI / 2, 0, 0] as const;

/** Converts a Z-up vector into Y-up coordinates. */
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
