// pxl-kernel/param.rs

//! The kernel's knobs: every tuned number nothing derives, with its reason,
//! grouped by the rung of the axis whose policy it tunes. None is a
//! correctness bound: set to its least, the view still converges, only
//! coarser or later.

mod chunk;
mod control;
mod io;
mod scene;
mod sched;
mod view;

pub use chunk::*;
pub use control::*;
pub use io::*;
pub use scene::*;
pub use sched::*;
pub use view::*;
