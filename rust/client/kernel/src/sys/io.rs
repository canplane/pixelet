// pxl-kernel/sys/io.rs

//! The kernel's I/O, below everything that asks for it. `host` is the host's
//! end: the calls a turn records and the answers that come back. `request` is
//! the one authority over what is out; `policy` orders it; `dispatch` hands
//! what it starts and stops to `fetch` and `loader`, which decode what
//! arrives, a body in the block `body` gives it; `room` is what a request
//! refused room waits on; `source` is where the tile source says they are.

pub mod body;
pub mod demand;
#[path = "io/demand.spec.rs"]
pub mod demand_spec;
pub mod dispatch;
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
pub mod room;
pub mod source;
pub mod text;
