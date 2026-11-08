// pxl-core/sys/io.rs

//! The core's I/O, below everything that asks for it. `request` is the one
//! authority over what is out, wanted by `demand`'s streams and ranked by
//! `policy`; `dispatch` hands what it starts and stops to `fetch` and
//! `loader`, which read through `file`, a body into the block `body` gives
//! it, and hand on what arrives. `room` is what a body refused room waits
//! on, and `failure` how a run ended. `source` is what the tile source
//! serves, `text` the platform's text, and `runtime` the door to the
//! runtime. Each `_spec` is generated from the JSON its header names.

pub mod body;
pub mod demand;
#[path = "io/demand.spec.rs"]
pub mod demand_spec;
pub mod dispatch;
pub mod failure;
pub mod fetch;
pub mod file;
#[path = "io/file.spec.rs"]
pub mod file_spec;
pub mod loader;
pub mod param;
pub mod policy;
pub mod request;
pub mod room;
pub mod runtime;
#[path = "io/runtime.spec.rs"]
pub mod runtime_spec;
pub mod source;
pub mod text;
#[path = "io/text.spec.rs"]
pub mod text_spec;
