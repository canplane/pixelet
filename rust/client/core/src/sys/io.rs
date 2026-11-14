// pxl-core/sys/io.rs

//! The core's I/O, beneath every part that asks for data. `request` is the
//! one authority over which reads are out: the streams of `demand` say what
//! is wanted, and `policy` ranks it. `dispatch` hands what `request` starts
//! and stops to `fetch` or `loader`, which read through `file` into memory
//! from `body` and pass on what arrives.

pub mod body;
pub mod demand;
pub mod dispatch;
pub mod failure;
pub mod fetch;
pub mod file;
pub mod loader;
pub mod param;
pub mod policy;
pub mod request;
pub mod room;
pub mod source;
pub mod text;
