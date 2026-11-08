// pxl-core/sys/io.rs

//! The core's I/O, beneath every part that asks for data. `request` is the
//! one authority over which reads are out: the streams of `demand` say what
//! is wanted, and `policy` ranks it. `dispatch` hands what `request` starts
//! and stops to `fetch` or `loader`, which read through `file` into blocks
//! from `body` and pass on what arrives. Each `_spec` module is generated
//! from the JSON its header names.

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
pub mod source;
pub mod text;
#[path = "io/text_answer.spec.rs"]
pub mod text_answer_spec;
#[path = "io/text.spec.rs"]
pub mod text_spec;
