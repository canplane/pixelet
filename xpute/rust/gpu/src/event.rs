// xpute-gpu/event.rs

//! What a core hears on the GPU's major, as the program's turn hands it a
//! command it took from the door.

use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;

use crate::abi::{Command, GpuSurfaceClose, GpuSurfaceResize};

pub enum Event {
    /// The GPU the core asked for has answered (GPU_READY), for the first
    /// time or again after a loss.
    Ready,
    Surface(Surface),
}

/// A surface is lent to the GPU by the UI (abi/gpu.json). It is opened, and the
/// program answers with its handle, or 0 for none; its box is resized; it is
/// let go.
#[derive(Clone, Copy, Debug)]
pub enum Surface {
    Open,
    Resize(GpuSurfaceResize),
    Close(GpuSurfaceClose),
}

/// What `cmd` says of the GPU, whose commands are the minors of the program's
/// `major`, and none for a command that is not the GPU's.
pub fn decode(major: u32, cmd: u32, pkt: Option<&[u8]>) -> Result<Option<Event>, MarshalError> {
    if cmd >> 8 != major {
        return Ok(None);
    }
    let body = || pkt.ok_or_else(|| MarshalError::new(Errno::EBADMSG));
    Ok(Some(match Command::of(cmd & 0xff) {
        Some(Command::GPU_READY) => Event::Ready,
        Some(Command::GPU_SURFACE_OPEN) => Event::Surface(Surface::Open),
        Some(Command::GPU_SURFACE_RESIZE) => {
            Event::Surface(Surface::Resize(GpuSurfaceResize::read(body()?)?))
        }
        Some(Command::GPU_SURFACE_CLOSE) => {
            Event::Surface(Surface::Close(GpuSurfaceClose::read(body()?)?))
        }
        None => return Ok(None),
    }))
}
