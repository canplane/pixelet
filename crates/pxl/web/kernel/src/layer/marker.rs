// pxl-kernel/layer/marker.rs

//! What is drawn to say where something is rather than to be part of the
//! world: the device's location puck, the ghost under the pointer, and the
//! dev gizmos a setting turns on.

pub mod debug_lines;
// Dev only: a published module carries none of its sketch or its labels.
#[cfg(feature = "dev")]
pub mod frame_gizmo;
pub mod hover_cursor;
pub mod location_puck;
