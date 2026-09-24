// pxl-kernel/layer/billboard.rs

//! Labels and bubbles drawn flat to the camera over the world: who is a
//! candidate this frame, which of them survive a shared visibility budget,
//! how they fade with distance, and the one sheet they all draw into. A
//! billboard is world-anchored, so the budget is what stops a dense place
//! from drawing a hundred of them.

pub mod batch;
pub mod canvas;
pub mod distance_fade;
pub mod field;
#[path = "billboard/order.spec.rs"]
pub mod order;
pub mod selection;
pub mod sheet;
