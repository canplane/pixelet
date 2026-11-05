// pxl-kernel/domain/scene/gizmo.rs

//! The debug views drawn over the world behind the `world_gizmo` setting: the frame's
//! and the coverage's gizmos, and the lines they are drawn in.

pub mod coverage_gizmo;
pub mod debug_line;
pub mod frame_gizmo;
#[path = "gizmo/gizmo.spec.rs"]
pub mod gizmo_spec;
