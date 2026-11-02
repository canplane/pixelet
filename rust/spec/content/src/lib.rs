// pxl-content/lib.rs

//! gpu-portable: pixelet's content, one source for the CPU and, through
//! rust-gpu, WGSL, as the engine's is (pxl-engine): the clouds' field
//! (`cloud`). The engine names none of it: pxl-spec, the kernel, the
//! renderer and the shader crate name it where they hand the engine its
//! content.

#![no_std]

pub mod cloud;
pub mod param;
