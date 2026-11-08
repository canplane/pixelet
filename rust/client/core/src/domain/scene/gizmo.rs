// pxl-core/domain/scene/gizmo.rs

//! The debug views drawn over the world while the `world_gizmo` setting is
//! on: the frame gizmo and the coverage gizmo, and the lines both are drawn
//! in.

pub mod coverage;
pub mod debug_line;
pub mod frame;
#[path = "gizmo/gizmo.spec.rs"]
pub mod gizmo_spec;
pub mod param;
