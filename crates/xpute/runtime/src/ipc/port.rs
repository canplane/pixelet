// xpute-runtime/ipc/port.rs

//! A command table: codec and dispatch, nothing about transport.
//!
//! A handler reads its args and body from the frame, writes its reply into
//! the same frame, and returns a spill if its body did not fit. It never
//! sets `RES`, never allocates a tx_id and never decides whether a reply is
//! sent — that is the runtime's (link.ts#serve) — which is what lets the same
//! table sit behind a worker, an in-memory link or, later, a WASM export.
//! The contract is April's CommandPort's, with the command numeric.

use xpute_core::errno::Errno;
use xpute_core::error::XputeError;
use xpute_core::kit::array::ArrayBuffer;
use xpute_core::log;

use super::frame::{Frame, FrameMessage};

/// Handles one request in place. Returns the reply's spill, or none; an
/// error is the reply's errno (a thrown XputeError there).
pub type CommandHandler = Box<dyn FnMut(&Frame, Option<ArrayBuffer>) -> Result<Option<ArrayBuffer>, XputeError>>;

#[derive(Default)]
pub struct CommandPort {
    handlers: Vec<Option<CommandHandler>>,
}

impl CommandPort {
    pub fn new() -> CommandPort {
        CommandPort { handlers: Vec::new() }
    }

    pub fn on(&mut self, cmd: u32, handler: CommandHandler) {
        if self.handlers.len() <= cmd as usize {
            self.handlers.resize_with(cmd as usize + 1, || None);
        }
        if self.handlers[cmd as usize].is_some() {
            panic!("ipc: command {cmd} registered twice");
        }
        self.handlers[cmd as usize] = Some(handler);
    }

    /// Runs the request's handler and returns the reply message, the frame
    /// rewritten in place. An unknown command replies ENOENT; a handler that
    /// errs replies with the XputeError's errno, and an empty reply — never
    /// a panic across the boundary. (The TypeScript catches anything thrown
    /// and replies EIO for what is not an XputeError; a handler here returns
    /// its error.)
    pub fn dispatch(&mut self, msg: FrameMessage) -> FrameMessage {
        let frame = Frame::new(msg.frame.clone());
        let handler = self.handlers.get_mut(frame.cmd() as usize).and_then(|h| h.as_mut());
        let mut spill: Option<ArrayBuffer> = None;
        let mut errno = Errno::OK;
        match handler {
            None => errno = Errno::ENOENT,
            Some(handler) => match handler(&frame, msg.spill) {
                Ok(s) => spill = s,
                Err(err) => {
                    errno = err.errno;
                    log::warn(&format!("[ipc] command {} failed: {err}", frame.cmd()));
                }
            },
        }
        if errno != Errno::OK {
            frame.lay_out(0, 0);
            spill = None;
        }
        frame.set_errno(errno as i32);
        FrameMessage { frame: msg.frame, spill }
    }
}
