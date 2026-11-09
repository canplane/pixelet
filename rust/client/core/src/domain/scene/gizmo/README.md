# Gizmos

A gizmo is a debug view, as an engine's editor draws its gizmos: it shows how
the core sees the world, each behind its setting, where an indicator
(`../indicator/`) is part of what a player is shown.

- `frame.rs`, behind the `world_gizmo` setting: the pivot's cell and the
  frames' rings, as debug lines;
- `coverage.rs`, behind the same setting: the eye's clip levels' bricks by
  how they stand, their tiles and shards, as translucent lines over the ground.

The lines are `debug_line.rs`'s, and their colors `gizmo.spec.rs`'s, generated
from `spec/client/core/domain/scene/gizmo/gizmo.json`.

Two views the renderer draws itself, from a word the core hands it, in the
looks of `spec/client/render/frame/shader/debug_look.json`:

- behind the `camera_gizmo` setting (`FrameInput::camera_gizmo`), the camera
  grid's cells over the screen and the cells the rays stop on tinted by the
  share of a grid cell their faces show the eye (pxl-render
  `frame/shader/camera_grid.wgsl`, `screen_shade.wgsl` `cell_share`);
- behind the `ray_steps` setting (pxl-engine `eye::STEPS`, `../cast.rs` `eye`),
  each ray's pixel colored by the steps taken on its way from the eye
  (pxl-render `frame/shader/cast.wgsl` `taken_word`, `queue.wgsl` `finish`).
