// pxl-shader/lib.rs

//! The GPU's compute entries rust-gpu makes, a module of their own each when
//! built: none now, every pass written by hand in pxl-render's WGSL. An entry
//! is named by `entry_point_name`, never by its module's path, so each keeps
//! the name pxl-tool shader.rs `ENTRIES` reads.

#![cfg_attr(target_arch = "spirv", no_std)]
// The workspace's clippy never builds this crate.
#![deny(warnings)]

// The panic handler a module built for the GPU needs, spirv-std's.
use spirv_std as _;
