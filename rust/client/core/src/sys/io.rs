// pxl-core/sys/io.rs

//! The core's I/O, beneath every part that asks for data. `request` is the
//! one authority over which reads are out: the streams of `demand` say what
//! is wanted, and `policy` ranks it. `dispatch` hands what `request` starts
//! and stops to `fetch` or `loader`, which read through `file` into memory
//! from `body` and pass on what arrives.

pub(crate) mod body;
pub(crate) mod demand;
pub(crate) mod dispatch;
pub(crate) mod failure;
pub(crate) mod fetch;
pub(crate) mod file;
pub(crate) mod loader;
pub(crate) mod param;
pub(crate) mod policy;
pub(crate) mod request;
pub(crate) mod room;
pub(crate) mod source;
pub(crate) mod text;
