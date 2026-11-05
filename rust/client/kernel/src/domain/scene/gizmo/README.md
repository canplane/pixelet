# Gizmos

A gizmo is a debug view, as an engine's editor draws its gizmos: it shows how
the kernel sees the world, each behind its setting, where an indicator
(`../indicator/`) is part of what a player is shown.

- `frame_gizmo.rs`, behind the `world_gizmo` setting: the pivot's cell and the
  frames' rings, as debug lines;
- `coverage_gizmo.rs`, behind the same setting: the eye's clip levels' bricks by
  how they stand, their tiles and shards, as translucent lines over the ground.

The lines are `debug_line.rs`'s, and their colors `gizmo.spec.rs`'s, generated
from `spec/client/kernel/domain/scene/gizmo/gizmo.json`.
