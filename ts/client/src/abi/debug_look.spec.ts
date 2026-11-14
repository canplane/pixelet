// @pxl/client/abi/debug_look.spec.ts
//
// GENERATED from spec/client/render/frame/shader/debug_look.json5 (xpute-gen data.rs)
//
// What the renderer's two debug views are drawn in, so the screen and the
// UI's legend of them read one look: the camera gizmo's grid and the tints
// of the cells the rays stop on (camera_grid.wgsl, screen_shade.wgsl
// `cell_share`), and the ray steps' ramp (screen_shade.wgsl). Each 0xrrggbb in
// sRGB.

/** The camera grid's lines. */
export const grid = {
  rgb: 16777215 as number,
  alpha: 0.25 as number,
} as const;

/** The tint on a cell the rays stop on, by the solid angle its face shows the
 * eye over a grid cell's, in this order, each up to its `most`. */
export const share = {
  /** Within one grid cell. */
  finer: {
    rgb: 5871103 as number,
    most: 1 as number,
  },
  /** Within one to four. */
  within: {
    rgb: 5895036 as number,
    most: 4 as number,
  },
  /** Past four. */
  coarser: {
    rgb: 16739417 as number,
  },
} as const;

/** The steps a pixel's ray took on its way from the eye, on a log scale. */
export const steps = {
  /** The steps the ramp runs up to, the same on every screen so two compare. */
  most: 1024 as number,
  /** A pixel that walked nothing, apart from the ramp's every color. */
  none: 8421504 as number,
  /** Google's Turbo, dark blue at none through green and yellow to dark red at
   * `most`, at even stops a pixel's color runs straight between. */
  ramp: [
    2299675 as number,
    4800175 as number,
    4483822 as number,
    3315191 as number,
    2538977 as number,
    2743739 as number,
    4256658 as number,
    6749549 as number,
    9894480 as number,
    13036347 as number,
    15650861 as number,
    16755492 as number,
    16744477 as number,
    15619093 as number,
    13184268 as number,
    10555906 as number,
    9440512 as number,
  ],
} as const;
