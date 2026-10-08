// pxl-kernel/mem.rs

//! Which sections the core memory is cut into, and what `.fixed` holds.

/// A mebibyte, the unit memory is reported in.
pub const MB: f64 = 1_048_576.0;

pub mod fixed;
pub mod section;
#[path = "mem/mem.spec.rs"]
pub mod spec;
