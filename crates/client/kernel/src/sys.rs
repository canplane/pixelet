// pxl-kernel/sys.rs

//! What all of the domain stands on: the reads (`io`), a turn's mechanics
//! (`sched`), the core memory's sections (`mem`), and what the kernel loads,
//! the table io and sched both read (`load`). Nothing here names the domain.

pub mod io;
pub mod load;
pub mod mem;
pub mod sched;
