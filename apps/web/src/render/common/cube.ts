// @pxl/web/render/common/cube.ts

// deno-fmt-ignore
export const CUBE_VERTICES = new Float32Array([
  -0.5, -0.5, -0.5,
   0.5, -0.5, -0.5,
   0.5,  0.5, -0.5,
  -0.5,  0.5, -0.5,
  -0.5, -0.5,  0.5,
   0.5, -0.5,  0.5,
   0.5,  0.5,  0.5,
  -0.5,  0.5,  0.5,
]);

// deno-fmt-ignore
export const CUBE_INDICES = new Uint16Array([
  0, 1, 2, 2, 3, 0,
  4, 6, 5, 6, 4, 7,
  0, 4, 5, 5, 1, 0,
  1, 5, 6, 6, 2, 1,
  2, 6, 7, 7, 3, 2,
  3, 7, 4, 4, 0, 3,
]);
