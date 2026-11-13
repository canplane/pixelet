// @pxl/client/abi/debug_look.spec.ts
//
// GENERATED from spec/client/render/frame/shader/debug_look.json (xpute-gen data.rs)
//
// What the renderer's two debug views are drawn in, so the screen and the
// UI's legend of them read one look: the camera gizmo's grid and the tints
// of the cells the rays stop on (camera_grid.wgsl, screen_shade.wgsl
// `cell_share`), and the ray steps' ramp (screen_shade.wgsl). Each #rrggbb in
// sRGB.

/** The camera grid's lines. */
export const grid = {
  rgb: "#ffffff",
  alpha: 0.25 as number,
} as const;

/** The tint on a cell the rays stop on, by the solid angle its face shows the
 * eye over a grid cell's, in this order, each up to its `most`. */
export const share = {
  finer: {
    rgb: "#5995ff",
    most: 1 as number,
  },
  within: {
    rgb: "#59f37c",
    most: 4 as number,
  },
  coarser: {
    rgb: "#ff6c59",
  },
} as const;

/** The steps a pixel's ray took on its way from the eye, on a log scale. */
export const steps = {
  most: 1024 as number,
  none: "#808080",
  /** Google's Turbo, dark blue at none through green and yellow to dark red at
   * `most`, at even stops a pixel's color runs straight between. */
  ramp: [
    "#23171b",
    "#493eaf",
    "#446aee",
    "#3295f7",
    "#26bde1",
    "#29ddbb",
    "#40f392",
    "#66fd6d",
    "#96fa50",
    "#c6eb3b",
    "#eed02d",
    "#ffab24",
    "#ff801d",
    "#ee5415",
    "#c92d0c",
    "#a11202",
    "#900d00",
  ],
} as const;
