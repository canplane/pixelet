// pxl-kernel/domain/scene/indicator.rs

//! What marks where something is: the location puck, the hover ghost, debug gizmos.

pub mod camera_gizmo;
pub mod coverage_gizmo;
pub mod debug_line;
pub mod frame_gizmo;
pub mod ghost;
#[path = "indicator/gizmo.spec.rs"]
pub mod gizmo_spec;
pub mod location_puck;
pub mod param;
