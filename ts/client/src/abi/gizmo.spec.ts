// @pxl/client/abi/gizmo.spec.ts
//
// GENERATED from spec/client/core/domain/scene/gizmo/gizmo.json (xpute-gen data.rs)
//
// What each line of the world gizmo is drawn in, so the lines and the
// UI's legend of them read one look: the frame's marks about the pivot
// (domain/scene/gizmo/frame.rs) and the coverage's over the ground
// (domain/scene/gizmo/coverage.rs). Each rgb is sRGB's #rrggbb, with its
// opacity, and dashed where a dashed line draws it: a ring of the frame, a
// stretch of the coverage.

export const frame = {
  pivot: {
    rgb: "#000000",
  },
  level: {
    rgb: "#000000",
    dashed: true,
  },
  forward: {
    rgb: "#ff0000",
    dashed: true,
  },
  north: {
    rgb: "#00ff00",
    dashed: true,
  },
  up: {
    rgb: "#0000ff",
  },
} as const;

export const coverage = {
  drawn: {
    rgb: "#ffffff",
    alpha: 0.4 as number,
    dashed: false,
  },
  drawn_from_floor: {
    rgb: "#00ffff",
    alpha: 0.6 as number,
    dashed: false,
  },
  unbuilt: {
    rgb: "#ffff00",
    alpha: 0.6 as number,
    dashed: true,
  },
  floor_only: {
    rgb: "#ff00ff",
    alpha: 0.6 as number,
    dashed: true,
  },
  no_tile: {
    rgb: "#ff0000",
    alpha: 0.6 as number,
    dashed: true,
  },
  tile: {
    rgb: "#ff9500",
    alpha: 0.9 as number,
    dashed: false,
  },
  shard: {
    rgb: "#7cff7c",
    alpha: 1 as number,
    dashed: false,
  },
} as const;
