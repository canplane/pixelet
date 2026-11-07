// xpute-core/abi/gpu.spec.rs
//
// GENERATED from spec/abi/gpu.json

//! The GPU device's protocol, on major GPU: its state and the surfaces a UI lends it. The
//! calls a core makes to the GPU are records of its stream (gpu/api.json), not commands.
//! GPU_READY tells the core the device the machine asked for answered. A surface is a
//! canvas or a view the UI hands over and the core alone draws from then on, by the handle
//! GPU_SURFACE_OPEN answers, the handle the UI binds the canvas under (@xpute/runtime
//! `Link.bind`) and the stream's CONFIGURE and CURRENT name. What it shows is the program's.

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

use crate::abi::number::{finite, integer, unknown};
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{PacketWriter, TreeReader};

/// What a command is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Major {
    GPU = 0x02,
}

impl Major {
    pub const ALL: [Major; 1] = [Major::GPU];

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<Major> {
        Major::ALL.into_iter().find(|m| *m as u32 == number)
    }
}

/// The GPU's state and its surfaces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum GpuMinor {
    READY = 0x01,
    SURFACE_OPEN = 0x02,
    SURFACE_RESIZE = 0x03,
    SURFACE_CLOSE = 0x04,
}

impl GpuMinor {
    pub const ALL: [GpuMinor; 4] = [
        GpuMinor::READY,
        GpuMinor::SURFACE_OPEN,
        GpuMinor::SURFACE_RESIZE,
        GpuMinor::SURFACE_CLOSE,
    ];

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<GpuMinor> {
        GpuMinor::ALL.into_iter().find(|m| *m as u32 == number)
    }
}

/// Every command, by the side that sends it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Command {
    // ---- gpu, export ----
    /// The GPU the machine asked for answered, the first time or again after a loss, whose
    /// sign is in the stream's device block.
    GPU_READY = 0x0201,
    /// Answered with the surface's handle, 0 where the core opens none.
    GPU_SURFACE_OPEN = 0x0202,
    /// The surface's layout box in device pixels, transforms aside, and the device pixel ratio;
    /// the buffer it is drawn into is the core's.
    GPU_SURFACE_RESIZE = 0x0203,
    /// The surface is going: what it drew and held on the device is let go.
    GPU_SURFACE_CLOSE = 0x0204,
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
}
