// pxl-kernel/io.rs

//! The kernel's I/O, below everything that asks for it. `host` is the host's
//! end: the calls a turn records, the answers that come back and what the host
//! says of itself. `request` is the one authority over what is out; `policy`
//! orders it; `fetch` and `load` decode what arrives; `source` is where the
//! tile source says they are.

pub mod demand;
pub mod failure;
pub mod fetch;
pub mod file;
pub mod host;
pub mod load;
pub mod policy;
pub mod request;
pub mod source;
#[path = "io/demand.spec.rs"]
pub mod spec;
pub mod text;
