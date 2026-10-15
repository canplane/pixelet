// pxl-kernel/scene/indicator.rs

//! What marks where something is: the location puck, the hover ghost, debug gizmos.

pub mod camera_gizmo;
pub mod debug_lines;
pub mod frame_gizmo;
#[path = "indicator/gizmo.spec.rs"]
pub mod gizmo_spec;
pub mod hover_cursor;
pub mod location_puck;
