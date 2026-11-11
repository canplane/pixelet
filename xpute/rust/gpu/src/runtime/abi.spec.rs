// xpute-gpu/runtime/abi.spec.rs
//
// GENERATED from spec/abi/gpu.json

//! The GPU device's protocol, which tells the core the device's state and the
//! surfaces the UI lends it. The core's own calls to the GPU are not commands: they are
//! records in its GPU stream (gpu/api.json). GPU_READY tells the core that the device its
//! core asked for has answered. A surface is a canvas or a view that the UI hands over,
//! and from then on only the core draws into it. The surface is named by the handle that
//! GPU_SURFACE_OPEN answers with: the UI binds its canvas under that handle (@xpute/gpu
//! `gpu_connect`), and the stream's CONFIGURE and CURRENT records name the surface by it. What
//! a surface shows is the program's to decide. A command's number here is its minor, under the
//! one major the program gives the GPU (xpute_gpu `abi::apply`, @xpute/gpu `gpu_exports`).

#![allow(
    non_camel_case_types,
    clippy::upper_case_acronyms,
    clippy::type_complexity,
    clippy::absolute_paths,
    unused_imports,
    dead_code,
    reason = "generated from the spec"
)]

use core::fmt::Display;

pub use crate::abi::Command;
use xpute_core::abi::number::{finite, integer, unknown};
use xpute_core::io::fs::{Path, Token};
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{BranchCursor, Cursor, PacketWriter, StrList, TreeReader};
use xpute_runtime::abi::{Export, Outcome};

/// Command::GPU_READY's packet.
pub struct GpuReady {}

impl GpuReady {
    /// The children it is written as.
    pub const FIELDS: u32 = 0;

    pub fn write(&self, _: &mut PacketWriter) {}
}

impl Export for GpuReady {
    const NUMBER: u32 = 1;
    const FIELDS: u32 = 0;
    fn write(&self, w: &mut PacketWriter) {
        GpuReady::write(self, w);
    }
}

/// Command::GPU_SURFACE_OPEN's packet.
pub struct GpuSurfaceOpen {}

impl GpuSurfaceOpen {
    /// The children it is written as.
    pub const FIELDS: u32 = 0;

    pub fn write(&self, _: &mut PacketWriter) {}
}

impl Export for GpuSurfaceOpen {
    const NUMBER: u32 = 2;
    const FIELDS: u32 = 0;
    fn write(&self, w: &mut PacketWriter) {
        GpuSurfaceOpen::write(self, w);
    }
}

/// Command::GPU_SURFACE_RESIZE's packet.
pub struct GpuSurfaceResize {
    pub surface: u32,
    pub width: u32,
    pub height: u32,
    pub device_pixel_ratio: f64,
}

impl GpuSurfaceResize {
    /// The children it is written as.
    pub const FIELDS: u32 = 4;

    pub fn write(&self, w: &mut PacketWriter) {
        w.u32(Some(self.surface));
        w.u32(Some(self.width));
        w.u32(Some(self.height));
        w.f64(Some(self.device_pixel_ratio));
    }
}

impl Export for GpuSurfaceResize {
    const NUMBER: u32 = 3;
    const FIELDS: u32 = 4;
    fn write(&self, w: &mut PacketWriter) {
        GpuSurfaceResize::write(self, w);
    }
}

/// Command::GPU_SURFACE_CLOSE's packet.
pub struct GpuSurfaceClose {
    pub surface: u32,
}

impl GpuSurfaceClose {
    /// The children it is written as.
    pub const FIELDS: u32 = 1;

    pub fn write(&self, w: &mut PacketWriter) {
        w.u32(Some(self.surface));
    }
}

impl Export for GpuSurfaceClose {
    const NUMBER: u32 = 4;
    const FIELDS: u32 = 1;
    fn write(&self, w: &mut PacketWriter) {
        GpuSurfaceClose::write(self, w);
    }
}
