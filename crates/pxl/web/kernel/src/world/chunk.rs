// pxl-kernel/world/chunk.rs

//! The world as chunks: what one holds, which ones a view needs, the tier
//! each is built at, and the raycast that finds what a ray met. This is where
//! a page's cells become something a frame can draw.

pub mod build;
pub mod mesh_slot;
pub mod raycast;
pub mod source;
pub mod stage;
pub mod table;
