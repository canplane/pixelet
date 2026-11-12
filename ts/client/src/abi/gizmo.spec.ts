// @pxl/client/abi/gizmo.spec.ts
//
// GENERATED from spec/client/core/domain/scene/gizmo/gizmo.json
//
// What each line of the world gizmo is drawn in, so the lines and the
// UI's legend of them read one look: the frame's marks about the pivot
// (domain/scene/gizmo/frame.rs) and the coverage's over the ground
// (domain/scene/gizmo/coverage.rs). Each rgb is sRGB's 0xRRGGBB as a
// number, with its opacity, and dashed where a dashed line draws it: a
// ring of the frame, a stretch of the coverage.

export const frame = {
  pivot: {
    rgb: 0 as number,
  },
  level: {
    rgb: 0 as number,
    dashed: true,
  },
  forward: {
    rgb: 16711680 as number,
    dashed: true,
  },
  north: {
    rgb: 65280 as number,
    dashed: true,
  },
  up: {
    rgb: 255 as number,
  },
} as const;

export const coverage = {
  drawn: {
    rgb: 16777215 as number,
    alpha: 0.4 as number,
    dashed: false,
  },
  drawn_from_floor: {
    rgb: 65535 as number,
    alpha: 0.6 as number,
    dashed: false,
  },
  unbuilt: {
    rgb: 16776960 as number,
    alpha: 0.6 as number,
    dashed: true,
  },
  floor_only: {
    rgb: 16711935 as number,
    alpha: 0.6 as number,
    dashed: true,
  },
  no_tile: {
    rgb: 16711680 as number,
    alpha: 0.6 as number,
    dashed: true,
  },
  tile: {
    rgb: 16749824 as number,
    alpha: 0.9 as number,
    dashed: false,
  },
  shard: {
    rgb: 8191868 as number,
    alpha: 1 as number,
    dashed: false,
  },
} as const;
