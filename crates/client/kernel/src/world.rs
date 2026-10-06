// pxl-kernel/world.rs

//! The world the kernel keeps: resident tiles, the query over them, the
//! chunks meshed from them, and the frame's view.

pub mod change;
pub mod chunk;
pub mod pyramid;
pub mod query;
pub mod view;

#[cfg(test)]
#[path = "world/fixture.test.rs"]
pub(crate) mod fixture;
