// pxl-kernel/world/chunk.rs

//! The world as chunks: what one holds, which ones a view needs, and the
//! tier each is built at. This is where a page's cells become something a
//! frame can draw.

pub mod build;
pub mod frontier;
pub mod mesh_slot;
pub mod source;
pub mod stage;
pub mod table;
