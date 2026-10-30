// @pxl/client/abi/debug_look.spec.ts
//
// GENERATED from spec/client/render/frame/shader/debug_look.json
//
// What the renderer's two debug views are drawn in, so the screen and the
// shell's legend of them read one look: the camera gizmo's grid and the
// tints of the cells the rays stop on (camera_grid.wgsl, screen_lit.wgsl
// `cell_share`), and the ray steps' ramp (screen_lit.wgsl). Each #rrggbb in
// sRGB.

/** The camera grid's lines. */
export const grid = {
  rgb: 0xffffff,
  alpha: 0.55 as number,
} as const;

/** The tint on a cell the rays stop on, by the solid angle its face shows the
 * eye over a grid cell's, in this order, each up to its `most`. */
export const share = {
  finer: {
    rgb: 0x5995ff,
    most: 1 as number,
  },
  within: {
    rgb: 0x59f37c,
    most: 4 as number,
  },
  coarser: {
    rgb: 0xff6c59,
  },
} as const;

/** The nodes a pixel's ray asked, on a log scale. */
export const steps = {
  most: 1024 as number,
  none: 0x808080,
  /** Google's Turbo, dark blue at none through green and yellow to dark red at
   * `most`, at even stops a pixel's color runs straight between. */
  ramp: [
    0x23171b, 0x493eaf, 0x446aee, 0x3295f7, 0x26bde1, 0x29ddbb, 0x40f392,
    0x66fd6d, 0x96fa50, 0xc6eb3b, 0xeed02d, 0xffab24, 0xff801d, 0xee5415,
    0xc92d0c, 0xa11202, 0x900d00,
  ],
} as const;
