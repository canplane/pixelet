// xpute-runtime/abi/fs.spec.rs
//
// GENERATED from spec/abi/fs.json

//! The file device's protocol on major FS: a core's files, read whole into its own buffers
//! and written whole, through a descriptor, as WASI's path_open, fd_read, fd_write and
//! fd_close do. A file's name is an absolute path, a tuple of tokens, each an i32
//! or a string, as many as the program uses. xpute gives no token a meaning, and the
//! program's one binding answers every path (@xpute/runtime io/fs.ts).
//!
//! The core sends its calls as imports. The core issues each descriptor, a generational
//! handle, and every frame of a descriptor carries it once, as its tag. FS_OPEN and FS_CLOSE
//! go with ACKREQ, and the runtime completes each once, on a later turn, with a frame of the
//! same tag and command, flagged RES. It serves a descriptor's frames in the order they came,
//! so an FS_READ is done before its close is completed, and the memory an FS_READ names is the
//! runtime's until then. An FS_WRITE's bytes go with its frame, and the file is replaced by
//! them at FS_CLOSE. The runtime tells the core that a file changed under it with
//! FS_CHANGED, an export. xpute ships both ends: the core's calls (xpute-core io/door.rs,
//! io/file.rs) and the runtime's server (@xpute/runtime). A program only gives the binding
//! that backs its paths.

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

use crate::abi::{Export, Outcome};
pub use xpute_core::abi::fs::{Command, FsMinor, Major};
use xpute_core::abi::number::{finite, integer, unknown};
use xpute_core::io::fs::{Path, Token};
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{BranchCursor, Cursor, PacketWriter, StrList, TreeReader};

/// Command::FS_CHANGED's packet.
pub struct FsChanged<'a> {
    /// The file's path, as the core opened it.
    pub path: &'a [Token<'a>],
}

impl FsChanged<'_> {
    /// The children it is written as.
    pub const FIELDS: u32 = 1;

    pub fn write(&self, w: &mut PacketWriter) {
        Path::write(w, self.path);
    }
}

impl Export for FsChanged<'_> {
    const NUMBER: u32 = 773;
    const FIELDS: u32 = 1;
    fn write(&self, w: &mut PacketWriter) {
        FsChanged::write(self, w);
    }
}

/// Command::FS_OPEN's completion's packet.
pub struct FsOpenReply {
    /// The number the runtime refused the open with, such as an HTTP status. It is kept to be
    /// printed, and nothing should branch on it.
    pub detail: u32,
}

impl FsOpenReply {
    /// The children it is written as.
    pub const FIELDS: u32 = 1;

    pub fn write(&self, w: &mut PacketWriter) {
        w.u32(Some(self.detail));
    }
}

/// Command::FS_OPEN's packet.
#[derive(Clone, Copy, Debug)]
pub struct FsOpen<'a> {
    pub path: Path<'a>,
    /// One of the `flag` values of io/fs.json.
    pub flags: u32,
}

impl<'a> FsOpen<'a> {
    pub fn read(pkt: &'a [u8]) -> Result<FsOpen<'a>, MarshalError> {
        let f = TreeReader::new(pkt)?.read_branch()?;
        Ok(FsOpen {
            path: Path::read(pkt, f.at(0)?)?,
            flags: integer::<u32>(f.at(1)?.get_number()?)?,
        })
    }
}

/// Command::FS_READ's packet.
#[derive(Clone, Copy, Debug)]
pub struct FsRead {
    pub at: u32,
    pub len: u32,
}

impl FsRead {
    pub fn read(pkt: &[u8]) -> Result<FsRead, MarshalError> {
        let f = TreeReader::new(pkt)?.read_branch()?;
        Ok(FsRead {
            at: integer::<u32>(f.at(0)?.get_number()?)?,
            len: integer::<u32>(f.at(1)?.get_number()?)?,
        })
    }
}

/// Command::FS_WRITE's packet.
#[derive(Clone, Copy, Debug)]
pub struct FsWrite {
    pub at: u32,
    pub len: u32,
}

impl FsWrite {
    pub fn read(pkt: &[u8]) -> Result<FsWrite, MarshalError> {
        let f = TreeReader::new(pkt)?.read_branch()?;
        Ok(FsWrite {
            at: integer::<u32>(f.at(0)?.get_number()?)?,
            len: integer::<u32>(f.at(1)?.get_number()?)?,
        })
    }
}

/// The majors the runtime routes the protocol's imports by, to `serve`.
pub const MAJORS: [u32; 1] = [Major::FS as u32];

/// What the runtime does with each command the core posts.
pub trait Imports {
    fn fs_open(&mut self, fd: u32, e: FsOpen<'_>) -> Outcome;
    fn fs_read(&mut self, fd: u32, e: FsRead) -> Outcome;
    fn fs_write(&mut self, fd: u32, e: FsWrite, named: &[u8]) -> Outcome;
    fn fs_close(&mut self, fd: u32) -> Outcome;
}

/// `cmd` read and handed to `h`; ENOSYS for one this protocol does not carry.
pub fn serve<H: Imports>(
    h: &mut H,
    tag: u32,
    cmd: u32,
    pkt: &Option<&[u8]>,
    named: &[u8],
) -> Outcome {
    match cmd {
        x if x == Command::FS_OPEN as u32 => h.fs_open(
            tag,
            FsOpen::read(pkt.ok_or_else(|| MarshalError::new(Errno::EBADMSG))?)?,
        ),
        x if x == Command::FS_READ as u32 => h.fs_read(
            tag,
            FsRead::read(pkt.ok_or_else(|| MarshalError::new(Errno::EBADMSG))?)?,
        ),
        x if x == Command::FS_WRITE as u32 => h.fs_write(
            tag,
            FsWrite::read(pkt.ok_or_else(|| MarshalError::new(Errno::EBADMSG))?)?,
            named,
        ),
        x if x == Command::FS_CLOSE as u32 => h.fs_close(tag),
        _ => Err(MarshalError::new(Errno::ENOSYS)),
    }
}
