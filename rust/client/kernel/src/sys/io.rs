// pxl-kernel/sys/io.rs

//! The kernel's I/O, below everything that asks for it. `runtime` is the runtime's
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
/// The kernel's roots and files, generated from file.json.
#[path = "io/file.spec.rs"]
pub mod file_spec;
pub mod loader;
pub mod param;
pub mod policy;
pub mod request;
pub mod room;
pub mod runtime;
/// The kernel's own calls, generated from runtime.json.
#[path = "io/runtime.spec.rs"]
pub mod runtime_spec;
pub mod source;
pub mod text;
/// The text device's calls, generated from text.json.
#[path = "io/text.spec.rs"]
pub mod text_spec;
