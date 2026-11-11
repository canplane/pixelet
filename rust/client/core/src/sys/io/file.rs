// pxl-core/sys/io/file.rs

//! The core's files, over xpute's file table. The table keeps each
//! descriptor and its owner until its close is completed, so each completion
//! reaches its owner with what it came to (`on_opened`, `on_closed`), and a
//! completion for a descriptor its owner closed reaches no one. A file's path
//! is a tuple of tokens, as long as its branch of file.json's `path` tree
//! takes: each token names a child of the node before it, and a branch that
//! holds data takes the tokens left, a tile or a shard by its quadkey and an
//! asset by its load's id. No path is kept: each is built where it is opened
//! or written (`with_path`), and a received one is walked as it came
//! (`named`); what is kept is what a path is made from, an id or a slot.
//! Everything past the path, such as a version, a URL or a storage key, is
//! the client's to decide (ts/client/src/service/sys/fs/). A file is bytes,
//! and how a platform keeps them is up to its backend.
//!
//! The storage file holds the settings, at (STORAGE, SETTINGS) (file.json
//! `path`). It is a format byte (file.json `format`) followed by
//! what the settings keep, and it is read whole and written whole. A file of
//! another format, one its backend cannot read as bytes, or one larger than
//! any heap is refused: it reads as nothing, it is never written over, and it
//! is named to the client. The client then sets the file aside and empties
//! the slot (`on_storage_emptied`).

use core::fmt;

use pxl_spec::space::coord::zoom::LEAF_ZOOM;
use pxl_spec::space::layer::quadrant::Quadrant;
use xpute_core::abi::fs::{completion, Completion as FsCompletion};
use xpute_core::global::Global;
use xpute_core::io::door::Completion;
pub use xpute_core::io::file::OpenResult;
use xpute_core::io::file::{FileTable, StableBuf};
use xpute_core::io::fs::{self, Fd, Token};
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use crate::sys::io::body::{self, Packet};
use crate::sys::io::demand::{Source, Stream};
use crate::sys::io::file_spec::{self, format, path};
use crate::sys::io::param::MAX_CREDITS;
use crate::sys::io::room::Refusal;
use crate::sys::io::{fetch, loader};
use crate::sys::manifest::MANIFEST;
use crate::sys::sched::request_world_tick;

pub enum StorageRead {
    /// Nothing was kept, or the file was empty.
    None,
    /// What the owner kept, without the format byte.
    Bytes(Vec<u8>),
    /// A file is there but cannot be read here, because it was refused
    /// (`is_storage_refused`) or its read failed. Its owner must never write
    /// over it.
    Unreadable,
}

/// The owner of an open file. A `Fetch` is a tile's or a shard's read, by its
/// attempt; a `Load`, an asset's file, by its attempt; `Storage`, the
/// settings' file.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Owner {
    Fetch { key: u64, at_ms: f64 },
    Load { key: u64 },
    Storage,
}

/// What awaits a file's close, sent once the owner reads it whole or writes
/// it: a read, whose packet the file table holds until the close is
/// completed, or a write. A fetch is a body's; a load, by its want's id, and
/// the storage file, the heap's.
pub enum Request {
    Fetch { key: u64, at_ms: f64 },
    Load { id: u64 },
    Storage,
    Write,
}

// SAFETY: a packet is a block of the body slots or the heap; moving it moves
// its handle alone.
unsafe impl StableBuf for Packet {}

/// Text held inline, as a quadkey is written: the first `len` of `N` bytes,
/// zero past them.
#[derive(Clone, Copy, PartialEq)]
pub struct Text<const N: usize> {
    pub bytes: [u8; N],
    len: usize,
}

/// The longest quadkey, a leaf's.
pub const KEY_BYTES: usize = LEAF_ZOOM as usize;

const _: () = assert!(file_spec::LEAF_ZOOM == LEAF_ZOOM);

/// The id that names the objects' top among the object stream's ids. Every
/// other id is a tile's, and no tile's id is 0.
pub const TOP_ID: u64 = 0;

/// The state of the storage file's read, from its open until its owner takes
/// it.
enum ReadState {
    /// The heap refused the read room. The file is opened again once the heap
    /// gives some back, and no descriptor is held meanwhile.
    Refused(Refusal),
    /// The read is out, and its buffer is the file table's until the file's
    /// close is completed.
    Reading,
    Read(StorageRead),
}

/// The most files open at once, plus slot 0: one for each credit and one for
/// the storage file's read.
const OPEN: usize = MAX_CREDITS as usize + 2;

struct Fs {
    files: FileTable<Owner, Request, Packet, OPEN>,
    storage: Option<ReadState>,
    /// Whether the storage file has been refused since the client last
    /// emptied its slot.
    refused: bool,
}

static FS: Global<Fs> = Global::new();

fn fs() -> &'static mut Fs {
    FS.get(|| Fs {
        files: FileTable::new(),
        storage: None,
        refused: false,
    })
}

/// The storage file's path, its slot after STORAGE.
pub const STORAGE: [Token<'static>; 2] = [
    Token::U32(path::STORAGE::ID),
    Token::U32(path::STORAGE::SETTINGS::ID),
];

/// Opens the file at `path`.
pub fn open(path: &[Token], owner: Owner) {
    fs().files.open(path, owner);
}

/// Opens the storage file to be read whole, for `take_storage`.
pub fn open_storage() {
    open(&STORAGE, Owner::Storage);
}

/// Hands a completion of the file device's to what awaits it (`on_opened`,
/// `on_closed`); another protocol's goes by.
pub fn on_completion(c: &Completion) {
    match completion(c) {
        Some(FsCompletion::FsOpen { fd, res, reply }) => {
            on_opened(fd, res, reply.map_or(0, |r| r.detail));
        }
        Some(FsCompletion::FsClose { fd, res }) => on_closed(fd, res),
        None => {}
    }
}

/// Hands an open the runtime completed to its owner. The table finds the
/// owner and lets go of a failed open's descriptor. A file larger than the
/// heap every device is given could be held nowhere, so it is closed here
/// and its owner hears EFBIG.
fn on_opened(fd: Fd, res: Result<u32, Errno>, detail: u32) {
    let Some((owner, opened)) = fs().files.complete_open(fd, res, detail) else {
        return;
    };
    let opened = match opened {
        OpenResult::Bytes(n) if n > MANIFEST.min_heap_bytes => {
            close(fd);
            OpenResult::Failed {
                errnum: Errno::EFBIG,
                detail,
            }
        }
        opened => opened,
    };
    match owner {
        Owner::Fetch { key, at_ms } => fetch::on_opened(fd, key, at_ms, opened),
        Owner::Load { key } => loader::on_opened(fd, key, opened),
        Owner::Storage => on_storage_opened(fd, opened),
    }
}

fn on_storage_opened(fd: Fd, opened: OpenResult) {
    let read = match opened {
        OpenResult::Bytes(0) => {
            close(fd);
            ReadState::Read(StorageRead::None)
        }
        OpenResult::Bytes(n) => match read_bytes(fd, n, Request::Storage) {
            Ok(()) => ReadState::Reading,
            Err(refused) => ReadState::Refused(refused),
        },
        // Only an absent file means nothing was kept.
        OpenResult::Absent => ReadState::Read(StorageRead::None),
        // The file is too large for any heap, or its backend cannot read it
        // as bytes.
        OpenResult::Failed {
            errnum: Errno::EFBIG | Errno::EBADMSG,
            ..
        } => ReadState::Read(refuse()),
        // A reload left the open unserved: the file is opened again.
        OpenResult::Failed {
            errnum: Errno::ECANCELED,
            ..
        } => return open_storage(),
        OpenResult::Failed { .. } => ReadState::Read(StorageRead::Unreadable),
    };
    set_read(read);
}

/// The storage file's read, whose close completed with `read`: its bytes,
/// or, where the close failed, nothing read, and the file is opened again.
fn on_storage_read(read: Result<Packet, Errno>) {
    match read {
        Ok(bytes) => set_read(ReadState::Read(storage_of(&bytes))),
        Err(_) => open_storage(),
    }
}

fn set_read(read: ReadState) {
    fs().storage = Some(read);
    request_world_tick();
}

/// What the storage file held, once it is read whole, or None until then.
pub fn take_storage() -> Option<StorageRead> {
    let read = &mut fs().storage;
    match read.take()? {
        ReadState::Read(storage_read) => Some(storage_read),
        ReadState::Refused(refused) if refused.released() => {
            open_storage();
            None
        }
        state @ (ReadState::Reading | ReadState::Refused(_)) => {
            *read = Some(state);
            None
        }
    }
}

/// The storage file's bytes as its backend read them: the settings' bytes
/// where the format byte is theirs, refused where it is any other.
fn storage_of(bytes: &[u8]) -> StorageRead {
    match bytes.split_first() {
        None => StorageRead::None,
        Some((&format::SETTINGS, settings)) => StorageRead::Bytes(settings.to_vec()),
        Some(_) => refuse(),
    }
}

/// Refuses the storage file. The next snapshot names the refusal to the
/// client, which sets the file aside.
fn refuse() -> StorageRead {
    fs().refused = true;
    request_world_tick();
    StorageRead::Unreadable
}

pub fn is_storage_refused() -> bool {
    fs().refused
}

/// The client has set the refused storage file aside and emptied its slot,
/// so the settings may read the slot again and write it.
pub fn on_storage_emptied() {
    fs().refused = false;
}

/// Hands a close the runtime completed to what awaits it: a read's bytes to
/// its reader. A write's result reaches no one, as nothing writes a file
/// again.
fn on_closed(fd: Fd, res: Result<u32, Errno>) {
    let Some((request, pkt, res)) = fs().files.complete_close(fd, res) else {
        return;
    };
    // A read's packet is the table's until its close completes, a write holds
    // none.
    let read = || res.map(|()| pkt.or_bug(Errno::ENOTRECOVERABLE));
    match request {
        Request::Fetch { key, at_ms } => fetch::on_read(key, at_ms, read()),
        Request::Load { id } => loader::on_read(id, read()),
        Request::Storage => on_storage_read(read()),
        Request::Write => {}
    }
}

/// Reads the opened file `fd` of `bytes` whole into a heap block, for
/// `request`, and closes it; the block is handed back filled as the close is
/// completed (`on_closed`). Where the heap has no room the file is closed
/// unread, and the owner opens it again once `Refusal::released`.
pub fn read_bytes(fd: Fd, bytes: u32, request: Request) -> Result<(), Refusal> {
    match body::take(None, bytes) {
        Ok(pkt) => {
            read(fd, pkt, request);
            Ok(())
        }
        Err(refused) => {
            close(fd);
            Err(refused)
        }
    }
}

/// Reads the opened file `fd` whole into `pkt` and closes it; the packet is
/// handed back filled, with `request`, as the close is completed
/// (`on_closed`).
pub fn read(fd: Fd, pkt: Packet, request: Request) {
    fs().files.read_whole(fd, pkt, request);
}

/// Closes the open file `fd`, whose owner hears nothing more of it.
pub fn close(fd: Fd) {
    let _ = fs().files.close(fd);
}

/// The open file whose owner `owner` picks.
pub fn find(owner: impl Fn(&Owner) -> bool) -> Option<Fd> {
    fs().files.find(owner)
}

/// Replaces the file at `path` with `bytes`. Writes of one path land in the
/// order sent.
pub fn write(path: &[Token], bytes: &[u8]) {
    fs().files.write_whole(path, bytes, Request::Write);
}

/// Writes `bytes` whole as the storage file's contents, after its format
/// byte. A refused file is not written over.
pub fn write_storage(bytes: &[u8]) {
    if is_storage_refused() {
        return;
    }
    let whole = [&[format::SETTINGS][..], bytes].concat();
    write(&STORAGE, &whole);
}

impl<const N: usize> Text<N> {
    pub const EMPTY: Text<N> = Text {
        bytes: [0; N],
        len: 0,
    };

    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.bytes[..self.len]).or_bug(Errno::ENOTRECOVERABLE)
    }
}

impl<const N: usize> fmt::Write for Text<N> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let to = self
            .bytes
            .get_mut(self.len..self.len + s.len())
            .ok_or(fmt::Error)?;
        to.copy_from_slice(s.as_bytes());
        self.len += s.len();
        Ok(())
    }
}

/// Answers `f` of the path that `stream`'s want `id` opens, built here at the
/// length its branch takes: a tile or a shard by its quadkey after its
/// stream's token, the objects' top as (OBJECT, TOP), and a load by its file.
/// A load that is no file has no path.
pub fn with_path<R>(stream: Stream, id: u64, f: impl FnOnce(&[Token]) -> R) -> Option<R> {
    match stream.row().source {
        Source::Load => loader::of_id(id).map(|(_, load)| f(&load.path())),
        Source::Bake(_) | Source::Objects => Some(with_quadrant(stream.token()?, id, f)),
    }
}

/// Answers `f` of the path of the tile or shard `id` after `head`: its quadkey,
/// or, for `TOP_ID`, the objects' top.
pub fn with_quadrant<R>(head: u32, id: u64, f: impl FnOnce(&[Token]) -> R) -> R {
    if id == TOP_ID {
        return f(&[Token::U32(head), Token::U32(path::OBJECT::TOP::ID)]);
    }
    let mut key = Text::<KEY_BYTES>::EMPTY;
    Quadrant::of_id(id)
        .write_key(&mut key)
        .or_bug(Errno::EOVERFLOW);
    f(&[Token::U32(head), Token::Str(key.as_str())])
}

/// A file a received path names among those the core refuses and reads
/// again: the storage file, or an objects' file by its want's id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum File {
    Storage,
    Objects(u64),
}

/// The file `path` names, walked token by token as it came; none for any
/// other path. An objects' file is (OBJECT, quadkey), a shard's, or (OBJECT,
/// TOP), the top's (`with_quadrant`).
pub fn of_path(path: fs::Path<'_>) -> Option<File> {
    let mut tokens = path.iter();
    let file = match (tokens.next()?, tokens.next()?) {
        (Token::U32(path::STORAGE::ID), Token::U32(path::STORAGE::SETTINGS::ID)) => File::Storage,
        (Token::U32(path::OBJECT::ID), Token::U32(path::OBJECT::TOP::ID)) => File::Objects(TOP_ID),
        (Token::U32(path::OBJECT::ID), Token::Str(key)) => {
            File::Objects(Quadrant::of_key(key)?.id())
        }
        _ => return None,
    };
    tokens.next().is_none().then_some(file)
}

#[cfg(test)]
#[path = "file.test.rs"]
pub(crate) mod test;
