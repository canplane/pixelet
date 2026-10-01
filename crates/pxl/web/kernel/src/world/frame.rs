// pxl-kernel/world/frame.rs

//! The frame the world stands in: the anchor every range is measured from,
//! the camera and view that move over it, and what the kernel writes for a
//! drawn frame. A world this large cannot be drawn from the origin, so
//! everything here is relative to an anchor that remounts as the view moves.

pub mod coord;
pub mod resolution;
pub mod stage;
pub mod view;
