// pxl-kernel/mem.rs

//! The kernel's memory: which ranges the module's linear memory is cut into,
//! and what is inside the one that is fixed. Cutting it is the mechanism's
//! (xpute_runtime::mem); which ranges there are is decided here.

pub mod fixed;
pub mod heap;
pub mod section;
#[path = "mem/mem.spec.rs"]
pub mod spec;
