// pxl-kernel/io.rs

//! The kernel's I/O. `sched` is the one authority over what is out; `policy`
//! orders it; `fetch` and `load` decode what arrives.

pub mod demand;
pub mod failure;
pub mod fetch;
pub mod file;
pub mod load;
pub mod policy;
pub mod request;
#[path = "io/demand.spec.rs"]
pub mod spec;
pub mod text;
