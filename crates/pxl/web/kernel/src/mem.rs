// pxl-kernel/mem.rs

//! Which sections the linear memory is cut into, and what `.fixed` holds.

pub mod fixed;
pub mod heap;
pub mod section;
#[path = "mem/mem.spec.rs"]
pub mod spec;
