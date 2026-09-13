// @pxl/web/render/chunk_lod.ts

/**
 * The camera's frustum, as the frame block the module's scheduler reads it
 * (crates/pxl-core/src/sched.rs). Every chunk's measurement — where it is
 * and whether it is on screen — is the module's, through the anchor root's
 * composed `matrixWorld`; this only hands it the planes.
 */

import type { Camera, Frustum, Matrix4 } from "three";

/**
 * Refreshes `frustum` to `camera`'s current one. `scratch_matrix` is mutated
 * and reused so this costs no allocation per call.
 */
export function update_frustum(frustum: Frustum, camera: Camera, scratch_matrix: Matrix4): void {
  scratch_matrix.multiplyMatrices(camera.projectionMatrix, camera.matrixWorldInverse);
  frustum.setFromProjectionMatrix(scratch_matrix);
}
