// xpute-gpu/abi.spec.rs
//
// GENERATED from spec/abi/gpu.json

//! The GPU device's protocol, which tells the core the device's state and the
//! surfaces the UI lends it. The core's own calls to the GPU are not commands: they are
//! records in its GPU stream (gpu/api.json). GPU_READY tells the core that the device its
//! machine asked for has answered. A surface is a canvas or a view that the UI hands over,
//! and from then on only the core draws into it. The surface is named by the handle that
//! GPU_SURFACE_OPEN answers with: the UI binds its canvas under that handle (@xpute/gpu
//! `gpu_connect`), and the stream's CONFIGURE and CURRENT records name the surface by it. What
//! a surface shows is the program's to decide. A command's number here is its minor, under the
//! one major the program gives the GPU (xpute_gpu `decode`, @xpute/gpu `gpu_protocol`).

// A list a record carries is a function handing out its items, whose
// type is long to spell and is spelled once, here, for each. A table
// with no import or no enum read off the wire leaves a writer or a
// number's check unused.
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

use xpute_core::abi::number::{finite, integer, unknown};
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{PacketWriter, TreeReader};

/// Every command, by the side that sends it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Command {
    // ---- gpu, export ----
    /// The GPU the machine asked for has answered, for the first time or again after the device
    /// was lost. The STATUS word of the stream's device block tells the core the device's state.
    GPU_READY = 0x0001,
    /// Asks the core to open a surface. The core answers with the surface's handle, or with 0
    /// when it opens none.
    GPU_SURFACE_OPEN = 0x0002,
    /// The surface's layout box in device pixels, ignoring transforms, and the device pixel
    /// ratio. The size of the buffer the surface is drawn into is the core's to choose.
    GPU_SURFACE_RESIZE = 0x0003,
    /// The surface is going away, and the core lets go of what it drew and held on the device
    /// for it.
    GPU_SURFACE_CLOSE = 0x0004,
}

impl Command {
    pub const ALL: [Command; 4] = [
        Command::GPU_READY,
        Command::GPU_SURFACE_OPEN,
        Command::GPU_SURFACE_RESIZE,
        Command::GPU_SURFACE_CLOSE,
    ];

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<Command> {
        Command::ALL.into_iter().find(|m| *m as u32 == number)
    }
}

/// Command::GPU_SURFACE_RESIZE's packet.
#[derive(Clone, Copy, Debug)]
pub struct GpuSurfaceResize {
    pub surface: u32,
    pub width: u32,
    pub height: u32,
    pub device_pixel_ratio: f64,
}

impl GpuSurfaceResize {
    pub fn read(pkt: &[u8]) -> Result<GpuSurfaceResize, MarshalError> {
        let f = TreeReader::new(pkt)?.read_branch()?;
        Ok(GpuSurfaceResize {
            surface: integer::<u32>(f.at(0)?.get_number()?)?,
            width: integer::<u32>(f.at(1)?.get_number()?)?,
            height: integer::<u32>(f.at(2)?.get_number()?)?,
            device_pixel_ratio: finite(f.at(3)?.get_number()?)?,
        })
    }
    pub const FIELDS: u32 = 4;

    pub fn write(&self, w: &mut PacketWriter) {
        w.u32(Some(self.surface));
        w.u32(Some(self.width));
        w.u32(Some(self.height));
        w.f64(Some(self.device_pixel_ratio));
    }
}

/// Command::GPU_SURFACE_CLOSE's packet.
#[derive(Clone, Copy, Debug)]
pub struct GpuSurfaceClose {
    pub surface: u32,
}

impl GpuSurfaceClose {
    pub fn read(pkt: &[u8]) -> Result<GpuSurfaceClose, MarshalError> {
        let f = TreeReader::new(pkt)?.read_branch()?;
        Ok(GpuSurfaceClose {
            surface: integer::<u32>(f.at(0)?.get_number()?)?,
        })
    }
    pub const FIELDS: u32 = 1;

    pub fn write(&self, w: &mut PacketWriter) {
        w.u32(Some(self.surface));
    }
}
