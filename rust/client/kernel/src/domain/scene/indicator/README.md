# Indicators (the one-off indicator tier)

An indicator is one bespoke shape placed in the local world frame
(`../../view/frame/coord.rs`), tied to input or the device's state rather than
to anything the world keeps, and it does **not** face the camera. That last
trait is what separates it from a world-anchored billboard: the ghost box
(`ghost.rs`) and the location puck's zone ring and beacon (`location_puck.rs`)
sit and turn in the world's own space, and neither counter-rotates toward the
eye. Their colors are this tier's own, pxl-render `look/color.rs` `indicator`
(`ACTION`, `ERASE`), not a general palette.

The tier's place among the others is
[doc/minutes/archive/20260907-world-rendering-tiers.md](/doc/minutes/archive/20260907-world-rendering-tiers.md).

Three files here are not indicators but debug views, kept in this folder for
convenience:

- `frame_gizmo.rs`, behind the `world_gizmo` setting: the focus's cell and the
  frames' rings, as debug lines;
- `coverage_gizmo.rs`, behind the same setting: the eye's windows' squares by
  how they stand, their tiles and shards, as translucent lines over the ground;
- `camera_gizmo.rs`, behind the `camera_gizmo` setting: the camera grid's cells,
  drawn per pixel by the renderer (pxl-render `frame/shader/camera_grid.wgsl`),
  and the cubes the rays stop on tinted by the share of a cell their faces show
  the eye.
