// @pxl/client/abi/gizmo.spec.ts
//
// GENERATED from spec/client/core/domain/scene/gizmo/gizmo.json5 (xpute-gen data.rs)
//
// What each line of the world gizmo is drawn in, so the lines and the
// UI's legend of them read one look: the frame's marks about the pivot
// (domain/scene/gizmo/frame.rs) and the coverage's over the ground
// (domain/scene/gizmo/coverage.rs). Each rgb is sRGB's 0xrrggbb, with its
// opacity, and dashed where a dashed line draws it: a ring of the frame, a
// stretch of the coverage.

/** About the pivot, opaque. */
export const frame = {
  /** The pivot and the ground cell it floors to. */
  pivot: {
    rgb: 0 as number,
  },
  /** The level ring. */
  level: {
    rgb: 0 as number,
    dashed: true,
  },
  /** The upright ring facing the screen, and the view's forward on it. */
  forward: {
    rgb: 16711680 as number,
    dashed: true,
  },
  /** The upright ring through north, and north on it. */
  north: {
    rgb: 65280 as number,
    dashed: true,
  },
  /** The point straight up from the pivot, where the two upright rings cross. */
  up: {
    rgb: 255 as number,
  },
} as const;

/** Over the ground, in the order a stretch shared by several takes the last: a drawn piece faint under what marks something wrong. */
export const coverage = {
  drawn: {
    rgb: 16777215 as number,
    alpha: 0.4 as number,
    dashed: false,
  },
  /** A drawn piece read from the floor's tile alone. */
  drawn_from_floor: {
    rgb: 65535 as number,
    alpha: 0.6 as number,
    dashed: false,
  },
  /** A goal piece with a tile on its line, its brick not yet filled. */
  unbuilt: {
    rgb: 16776960 as number,
    alpha: 0.6 as number,
    dashed: true,
  },
  /** A goal piece only the floor's tile can build. */
  floor_only: {
    rgb: 16711935 as number,
    alpha: 0.6 as number,
    dashed: true,
  },
  /** A goal piece with no tile on its line. */
  no_tile: {
    rgb: 16711680 as number,
    alpha: 0.6 as number,
    dashed: true,
  },
  /** A drawn piece's tile. */
  tile: {
    rgb: 16749824 as number,
    alpha: 0.9 as number,
    dashed: false,
  },
  /** A drawn piece's shard, the unit the world's data is kept in. */
  shard: {
    rgb: 8191868 as number,
    alpha: 1 as number,
    dashed: false,
  },
} as const;
