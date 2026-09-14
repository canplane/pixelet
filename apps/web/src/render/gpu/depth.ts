// @pxl/web/render/gpu/depth.ts

/**
 * The depth convention every pass shares: reversed Z in a 32-bit float,
 * near at 1 and far at 0. A float's precision is densest near zero, and a
 * perspective projection's depth is densest near the eye, so the two
 * cancel and the far terrain's coplanar faces stop fighting — the fix
 * every engine reached for once a float depth format was there to take
 * it. GL's clip z is [-1, 1] with far at 1; the clip transform below maps
 * it to [1, 0].
 */

export const DEPTH_FORMAT: GPUTextureFormat = "depth32float";
/** Nearer is greater. */
export const DEPTH_COMPARE: GPUCompareFunction = "greater";
export const DEPTH_COMPARE_OR_EQUAL: GPUCompareFunction = "greater-equal";
/** The far plane, which nothing is behind. */
export const DEPTH_CLEAR = 0;
/** GL clip z [-1, 1] to reversed WebGPU clip z [1, 0], column-major. */
export const GL_TO_WGPU = new Float64Array([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, -0.5, 0, 0, 0, 0.5, 1]);
/** GL's clip z back from a reversed depth value in [0, 1]. */
export const DEPTH_TO_GL_WGSL = /* wgsl */ `
fn depth_to_gl_z(d: f32) -> f32 {
  return 1.0 - 2.0 * d;
}
`;
