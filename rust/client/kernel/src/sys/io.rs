// pxl-kernel/sys/io.rs

//! The kernel's I/O, below everything that asks for it. `host` is the host's
//! end: the calls a turn records, the answers that come back and what the host
//! says of itself. `request` is the one authority over what is out; `policy`
//! orders it; `fetch` and `loader` decode what arrives, a body in the block
//! `body` gives it; `source` is where the tile source says they are.

pub mod body;
pub mod demand;
#[path = "io/demand.spec.rs"]
pub mod demand_spec;
pub mod failure;
pub mod fetch;
pub mod file;
pub mod host;
/// The kernel's own calls, roots and files, generated from host.json.
#[path = "io/host.spec.rs"]
pub mod host_spec;
pub mod loader;
pub mod param;
pub mod policy;
pub mod request;
pub mod source;
pub mod text;
