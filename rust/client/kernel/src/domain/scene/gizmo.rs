// pxl-kernel/domain/scene/gizmo.rs

//! The debug views drawn over the world, each behind its setting: the frame's,
//! the coverage's and the camera's gizmos, and the lines they are drawn in.

pub mod camera_gizmo;
pub mod coverage_gizmo;
pub mod debug_line;
pub mod frame_gizmo;
#[path = "gizmo/gizmo.spec.rs"]
pub mod gizmo_spec;
