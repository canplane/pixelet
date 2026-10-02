// pxl-kernel/world/frame.rs

//! The frame the world stands in: everything is relative to a floating
//! origin that rebases as the view moves, since the world is too large to
//! draw from a fixed one.

pub mod coord;
pub mod resolution;
pub mod stage;
pub mod view;
