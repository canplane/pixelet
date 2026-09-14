// @pxl/web/render/gpu/depth.ts

/**
 * The depth convention every pass shares: reversed Z in a 32-bit float,
 * near at 1 and far at 0. A float's precision is densest near zero, and a
 * perspective projection's depth is densest near the eye, so the two
 * cancel and the far terrain's coplanar faces stop fighting — the fix
 * every engine reached for once a float depth format was there to take
 * it. GL's clip z is [-1, 1] with far at 1; the clip transform below maps
 * it to [1, 0]. The shaders' side of the convention is shader/depth.wgsl.
 */

export const DEPTH_FORMAT: GPUTextureFormat = "depth32float";
/** The far plane, which nothing is behind. */
export const DEPTH_CLEAR = 0;
/** GL clip z [-1, 1] to reversed WebGPU clip z [1, 0], column-major. */
export const GL_TO_WGPU = new Float64Array([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, -0.5, 0, 0, 0, 0.5, 1]);
