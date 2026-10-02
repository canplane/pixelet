// pxl-kernel/world.rs

//! The world the kernel keeps: resident pages, the query over them, the
//! chunks meshed from them, and the frame's view.

pub mod access;
pub mod change;
pub mod chunk;
pub mod frame;
pub mod observation;
pub mod observer;
pub mod paging;
pub mod prefetch;
pub mod pyramid;
pub mod quality;

#[cfg(test)]
#[path = "world/fixture.test.rs"]
pub(crate) mod fixture;
