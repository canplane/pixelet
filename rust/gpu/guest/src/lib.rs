// pxl-gpu-guest/lib.rs

//! This crate holds the GPU calls of a guest that cannot reach a GPU itself.
//! The guest writes WebGPU's calls as records of a stream (`encode`, `stream`)
//! and lends them to its host, which runs them on a device; spec/gpu/
//! gives each op's words and the enumerations they carry, and spec/gpu/wit/ the
//! link's interfaces: the host's `gpu` and its device, the guest's surfaces.
//! `admit` says whether the host takes another frame now. The host for a
//! native device is pxl-gpu-host. A program that draws nothing links
//! none of it.

pub mod abi;

pub mod admit;
pub mod device;
pub mod encode;
pub mod handle;
pub mod stream;
