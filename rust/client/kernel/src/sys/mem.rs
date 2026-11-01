// pxl-kernel/sys/mem.rs

//! Which sections the core memory is cut into, and what `.fixed` holds.

pub(crate) mod range;
pub(crate) mod section;
#[path = "mem/mem.spec.rs"]
pub mod spec;
