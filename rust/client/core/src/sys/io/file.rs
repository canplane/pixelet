// pxl-core/sys/io/file.rs

//! xpute's file table, an owner a descriptor, so an open's completion reaches
//! its owner and one for a descriptor since closed reaches no one. A file is
//! a root's object by one name (file.json `root`): a tile's or a shard's by
//! its quadkey, an asset by its load's id, the storage file by its slot, the
//! sphere and the objects' top and edits by their own. What it is called
//! past that, a version, a URL or a storage key, is the client's
//! (ts/client/src/sys/fs/). A file is bytes; how a platform keeps them is
//! its backend's. A native test has no runtime and opens nothing.
//!
//! The storage file is the settings', in their slot (file.json
//! `storage.SETTINGS`): its format byte (file.json `format`), then what the
//! settings keep, read whole and written whole. One of another format, one
//! its backend cannot read as bytes or one larger than any heap is refused:
//! read as nothing, never written over, and named to the client, whose it is
//! to set the file aside and empty the slot (`on_storage_emptied`).

use core::fmt::{self, Write};

use pxl_spec::space::layer::quadrant::{Quadrant, LEAF_ZOOM};
use xpute_core::global::Global;
use xpute_core::ipc::door::Pos;
pub use xpute_core::ipc::door::Reading;
use xpute_core::ipc::file::Files;
pub use xpute_core::ipc::file::Opened;
use xpute_core::ipc::fs::{Fd, FsOpened};
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use crate::machine::MACHINE;
use crate::sys::io::demand::{Source, Stream};
use crate::sys::io::file_spec::{format, object, root, storage};
use crate::sys::io::param::{MAX_CREDITS, WRITES_A_TURN};
use crate::sys::io::room::Refusal;
use crate::sys::io::runtime::{door, turn};
use crate::sys::io::{fetch, loader};
use crate::sys::sched::edge::request_world_tick;

pub enum StorageRead {
    /// None was kept, or it was empty.
    None,
    /// What its owner kept, past the format byte.
    Bytes(Vec<u8>),
    /// There and not readable here, refused (`is_storage_refused`) or failing
    /// otherwise: never written over.
    Unreadable,
}

/// `Load` is an asset's file, read as a model or an image as its attempt's
/// id says.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Owner {
    Fetch { key: u64, at_ms: f64 },
    Load { key: u64 },
    Storage,
    Write,
}

/// An object's path below its root, written in place, so no path is
/// allocated.
#[derive(Clone, Copy, PartialEq)]
pub struct Path {
    bytes: [u8; PATH_BYTES],
    len: usize,
}

/// The longest name the core opens: a u64 in decimal, at its widest, past
/// every quadkey.
pub const PATH_BYTES: usize = 20;

const _: () = assert!(u64::MAX.ilog10() as usize + 1 == PATH_BYTES);
const _: () = assert!(LEAF_ZOOM as usize <= PATH_BYTES);

/// The objects' top among a stream's ids, the rest tiles': no tile's is 0.
pub const TOP_ID: u64 = 0;

/// A storage file's read, from its open until its owner takes it.
enum ReadState {
    /// The heap refused its room: opened again once it gives some back, no
    /// descriptor kept meanwhile.
    Refused(Refusal),
    /// The runtime's until its head has passed the read.
    Reading(Reading),
    Read(StorageRead),
}

/// The most files open at once, and slot 0: one a credit, the storage file's
/// read, and a write, opened and closed in one call.
const OPEN: usize = MAX_CREDITS as usize + 3;

struct Fs {
    files: Files<Owner, OPEN>,
    storage: Option<ReadState>,
    /// The storage file was refused since the client last emptied its slot.
    refused: bool,
    /// The turn `write` last wrote in, and how many files it wrote there.
    written: (u64, u32),
}

static FS: Global<Fs> = Global::new();

fn fs() -> &'static mut Fs {
    FS.get(|| Fs {
        files: Files::new(),
        storage: None,
        refused: false,
        written: (0, 0),
    })
}

/// The storage file's path below STORAGE, its slot in decimal.
pub fn storage_path() -> Path {
    Path::of_id(u64::from(storage::SETTINGS))
}

/// Opens `root`'s object `path`; none where there is no runtime.
pub fn open(root: u32, path: Path, owner: Owner) -> Option<Fd> {
    Some(fs().files.open(door()?, root, path.as_str(), owner))
}

/// Opens the storage file to be read whole, for `take_storage`.
pub fn open_storage() -> Option<Fd> {
    open(root::STORAGE, storage_path(), Owner::Storage)
}

/// An open the runtime completed, read once; the table finds its owner and
/// lets a failed one's descriptor go. A file larger than the heap every
/// device is given is held nowhere: it is closed here, and its owner hears
/// EFBIG.
pub fn on_opened(completed: &FsOpened) {
    let FsOpened { fd, detail, .. } = *completed;
    let Some(owner) = door().and_then(|_| fs().files.complete_open(completed)) else {
        return;
    };
    let opened = match Opened::of(completed) {
        Opened::Bytes(n) if n > MACHINE.min_heap_bytes => {
            close(fd);
            Opened::Failed {
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
        Owner::Write => {}
    }
}

fn on_storage_opened(fd: Fd, opened: Opened) {
    let read = match opened {
        Opened::Bytes(0) => {
            close(fd);
            ReadState::Read(StorageRead::None)
        }
        Opened::Bytes(n) => match read_whole(fd, n) {
            Ok(reading) => ReadState::Reading(reading),
            Err(refused) => ReadState::Refused(refused),
        },
        // Only its absence says nothing was kept.
        Opened::Absent => ReadState::Read(StorageRead::None),
        // Too large for any heap, or there and not bytes to its backend.
        Opened::Failed {
            errnum: Errno::EFBIG | Errno::EBADMSG,
            ..
        } => ReadState::Read(refuse()),
        Opened::Failed { .. } => ReadState::Read(StorageRead::Unreadable),
    };
    set_read(read);
}

fn set_read(read: ReadState) {
    fs().storage = Some(read);
    request_world_tick();
}

/// What the storage file held, once it is read whole; none until then.
pub fn take_storage() -> Option<StorageRead> {
    let read = &mut fs().storage;
    match read.take()? {
        ReadState::Read(held) => Some(held),
        ReadState::Reading(reading) => match take_read(reading) {
            Ok(bytes) => Some(storage_of(bytes)),
            Err(reading) => {
                set_read(ReadState::Reading(reading));
                None
            }
        },
        ReadState::Refused(refused) if refused.over() => {
            open_storage();
            None
        }
        refused @ ReadState::Refused(_) => {
            *read = Some(refused);
            None
        }
    }
}

/// The storage file as its backend read it: the settings' bytes where it is
/// of their format, refused where it is of any other.
fn storage_of(mut bytes: Vec<u8>) -> StorageRead {
    if bytes.is_empty() {
        return StorageRead::None;
    }
    if bytes[0] != format::SETTINGS {
        return refuse();
    }
    bytes.remove(0);
    StorageRead::Bytes(bytes)
}

/// Named to the client in the next snapshot, for it to set aside.
fn refuse() -> StorageRead {
    fs().refused = true;
    request_world_tick();
    StorageRead::Unreadable
}

pub fn is_storage_refused() -> bool {
    fs().refused
}

/// The client set the refused storage file aside and emptied its slot: the
/// settings may read it again and write it.
pub fn on_storage_emptied() {
    fs().refused = false;
}

/// Posts a read of `fd` into `dst`, filled once the runtime's head has
/// passed it (`is_consumed`).
///
/// # Safety
/// As `Door::read`: `dst` stays allocated, unmoved and unread until
/// `is_consumed` says the runtime is done with it.
pub unsafe fn read(fd: Fd, dst: &mut [u8]) -> Pos {
    let door = door().or_bug(Errno::ENOTRECOVERABLE);
    // SAFETY: the caller's.
    unsafe { door.read(fd, dst) }
}

pub fn is_consumed(pos: Pos) -> bool {
    door().or_bug(Errno::ENOTRECOVERABLE).is_consumed(pos)
}

/// An opened file of `bytes`, read whole and closed; refused where the heap
/// has no room, for its owner to open again once `Refusal::over`.
pub fn read_whole(fd: Fd, bytes: u32) -> Result<Reading, Refusal> {
    let door = door().or_bug(Errno::ENOTRECOVERABLE);
    fs().files
        .read_whole(door, fd, bytes)
        .map_err(|_| Refusal::now())
}

/// The bytes `reading` holds once the runtime's head has passed its read;
/// the reading again before then.
pub fn take_read(reading: Reading) -> Result<Vec<u8>, Reading> {
    reading.take(door().or_bug(Errno::ENOTRECOVERABLE))
}

pub fn close(fd: Fd) {
    if let Some(door) = door() {
        fs().files.close(door, fd);
    }
}

pub fn find(owner: impl Fn(&Owner) -> bool) -> Option<Fd> {
    fs().files.find(owner)
}

/// Writes `bytes` whole as `root`'s object `path`, where the turn has a
/// write left (`WRITES_A_TURN`); false where it has none, for its owner to
/// write on a later turn. Where there is no runtime nothing is written, and
/// none counted.
pub fn write(root: u32, path: Path, bytes: Vec<u8>) -> bool {
    let Some(door) = door() else { return true };
    let f = fs();
    if f.written.0 != turn() {
        f.written = (turn(), 0);
    }
    if f.written.1 >= WRITES_A_TURN {
        return false;
    }
    f.written.1 += 1;
    f.files
        .write_whole(door, root, path.as_str(), bytes, Owner::Write);
    true
}

/// Writes `bytes` whole as what the storage file holds, its format first; a
/// refused file is not written over.
pub fn write_storage(bytes: &[u8]) {
    if is_storage_refused() {
        return;
    }
    let Some(door) = door() else { return };
    let whole = [&[format::SETTINGS][..], bytes].concat();
    fs().files.write_whole(
        door,
        root::STORAGE,
        storage_path().as_str(),
        whole,
        Owner::Write,
    );
}

impl Path {
    pub const EMPTY: Path = Path {
        bytes: [0; PATH_BYTES],
        len: 0,
    };

    /// A load's or a slot's, in decimal.
    pub fn of_id(id: u64) -> Path {
        let mut p = Path::EMPTY;
        write!(p, "{id}").or_bug(Errno::EOVERFLOW);
        p
    }

    pub fn of_name(name: &str) -> Path {
        let mut p = Path::EMPTY;
        p.write_str(name).or_bug(Errno::EOVERFLOW);
        p
    }

    /// A tile's or a shard's quadkey by its id, or the objects' top.
    pub fn of_quadrant(id: u64) -> Path {
        if id == TOP_ID {
            return Path::of_name(object::TOP);
        }
        let mut p = Path::EMPTY;
        Quadrant::of_id(id)
            .write_key(&mut p)
            .or_bug(Errno::EOVERFLOW);
        p
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }

    pub fn as_str(&self) -> &str {
        core::str::from_utf8(self.as_bytes()).or_bug(Errno::ENOTRECOVERABLE)
    }
}

impl Write for Path {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let end = self.len + s.len();
        let to = self.bytes.get_mut(self.len..end).ok_or(fmt::Error)?;
        to.copy_from_slice(s.as_bytes());
        self.len = end;
        Ok(())
    }
}

impl fmt::Display for Path {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The path `stream`'s want `id` opens: a load's, or a tile's or a shard's.
pub fn path_of(stream: Stream, id: u64) -> Path {
    match stream.row().source {
        Source::Load => loader::of_id(id).map_or(Path::EMPTY, |(_, load)| load.path()),
        Source::Bake(_) | Source::Objects => Path::of_quadrant(id),
    }
}

/// The id a path below OBJECT names (`Path::of_quadrant`); none for what is
/// no shard and not the top.
pub fn id_of_path(path: &str) -> Option<u64> {
    if path == object::TOP {
        return Some(TOP_ID);
    }
    Quadrant::of_key(path).map(Quadrant::id)
}

#[cfg(test)]
#[path = "file.test.rs"]
pub(crate) mod test;
