// pxl-core/sys/io/file.rs

//! xpute's file table with an owner per descriptor, so an open's completion
//! reaches its owner and one for a closed descriptor reaches no one. A
//! native test has no runtime and opens nothing.
//!
//! Every file is a root's object by one name (file.json `root`): a tile and a
//! shard's objects by the quadkey, an asset by its load's id, a storage file by
//! its slot's, the sphere, the objects' top and their edits by their own. What
//! it is called past that and where, a version, a URL or a storage key, is what
//! the client
//! (ts/client/src/sys/fs/) binds the root to, `@xpute/runtime`
//! carrying the open there. A file is bytes: how a platform keeps them, as
//! text or otherwise, is its backend's.

use core::fmt::{self, Write};
use pxl_spec::space::layer::quadrant::{Quadrant, LEAF_ZOOM};
use xpute_core::ipc::door::Pos;
pub use xpute_core::ipc::door::Reading;
use xpute_core::ipc::file::Files;
pub use xpute_core::ipc::file::Opened;
use xpute_core::ipc::fs::{Fd, FsOpened};
use xpute_kit::status::bug::OrBug;

use crate::machine::MACHINE;
use crate::sys::io::demand::{Source, Stream};
use crate::sys::io::fetch;
use crate::sys::io::file_spec::{format, object, root, storage};
use crate::sys::io::loader;
use crate::sys::io::param::{MAX_CREDITS, WRITES_A_TURN};
use crate::sys::io::request::Credit;
use crate::sys::io::room::Refusal;
use crate::sys::io::runtime::{door, turn};
use crate::sys::sched::edge::request_world_tick;
use xpute_core::global::Global;
use xpute_kit::status::errno::Errno;

/// A file the core keeps in storage, by its slot (file.json `storage`).
/// Storage is opened and written only by one of these, so what a turn can
/// record there follows from how many.
///
/// A storage file is its slot's format byte (file.json `format`), then what its
/// owner keeps. One of another format, one its backend cannot read as bytes
/// (EBADMSG), or one larger than any heap a device is given (no build reads
/// it) is refused: read as nothing and never written over, and named to the
/// client (`is_refused`), whose it is to set the file aside and empty the slot
/// (`on_emptied`). Which storage a slot is, how it keeps the bytes, and what
/// an older file becomes, the core never says.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StorageFile {
    Settings,
}

impl StorageFile {
    /// In declaration order, as `READS` is indexed by `file as usize`.
    pub const ALL: [StorageFile; 1] = [StorageFile::Settings];

    /// Its path below STORAGE is this in decimal.
    pub const fn slot(self) -> u32 {
        match self {
            StorageFile::Settings => storage::SETTINGS,
        }
    }

    pub const fn format(self) -> u8 {
        match self {
            StorageFile::Settings => format::SETTINGS,
        }
    }

    pub fn of_slot(slot: u32) -> Option<StorageFile> {
        StorageFile::ALL.into_iter().find(|k| k.slot() == slot)
    }

    pub fn path(self) -> Path {
        Path::of_id(u64::from(self.slot()))
    }
}

/// The longest name the core opens: a u64 in decimal, at its widest, past
/// every quadkey, so a slot holds the open that names it.
pub const PATH_BYTES: usize = 20;

const _: () = assert!(u64::MAX.ilog10() as usize + 1 == PATH_BYTES);
const _: () = assert!(LEAF_ZOOM as usize <= PATH_BYTES);

/// The objects' top among a stream's ids, the rest tiles' keys: no tile's is 0.
pub const TOP_ID: u64 = 0;

/// `Load` is an asset's file, read as a model or an image as its attempt's
/// id says.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Owner {
    Fetch { key: u64, at_ms: f64 },
    Load { key: u64 },
    StorageFile(StorageFile),
    Write,
}

/// The most files open at once, and slot 0: a credit's each (`open` takes
/// one), each storage file's read, and a write, opened and closed in one call.
const OPEN: usize = MAX_CREDITS as usize + StorageFile::ALL.len() + 2;

static FILES: Global<Files<Owner, OPEN>> = Global::new();

fn files() -> &'static mut Files<Owner, OPEN> {
    FILES.get(Files::new)
}

/// Opens `root`'s object `path`; none where there is no runtime.
pub fn open(_: Credit, root: u32, path: Path, owner: Owner) -> Option<Fd> {
    Some(files().open(door()?, root, path.as_str(), owner))
}

/// Opens a storage file to be read whole; what it held is taken with `take_storage`.
pub fn open_storage(file: StorageFile) -> Option<Fd> {
    Some(files().open(
        door()?,
        root::STORAGE,
        file.path().as_str(),
        Owner::StorageFile(file),
    ))
}

/// What a storage file held.
pub enum StorageRead {
    /// None was kept, or it was empty.
    None,
    /// What its owner kept, past the format byte.
    Bytes(Vec<u8>),
    /// There, and not readable here: an error other than its absence, or
    /// refused (`is_refused`). It must not be written over.
    Unreadable,
}

/// The slots refused since they were last emptied, by `StorageFile`.
static REFUSALS: Global<[bool; StorageFile::ALL.len()]> = Global::new();

fn refusals() -> &'static mut [bool; StorageFile::ALL.len()] {
    REFUSALS.get(|| [false; StorageFile::ALL.len()])
}

/// Its file is not of its slot's format, or too large for any heap: none of
/// it was read, and nothing is written over it until the client empties the slot.
pub fn is_refused(file: StorageFile) -> bool {
    refusals()[file as usize]
}

/// The client set a refused slot's file aside and emptied it: its owner may
/// open it again and write it.
pub fn on_emptied(file: StorageFile) {
    refusals()[file as usize] = false;
}

/// A storage file as its backend read it, `bytes`: its owner's bytes where it
/// is of its slot's format, refused where it is anything else.
pub fn storage_of(file: StorageFile, mut bytes: Vec<u8>) -> StorageRead {
    if bytes.is_empty() {
        return StorageRead::None;
    }
    if bytes.first() != Some(&file.format()) {
        return refuse(file);
    }
    bytes.remove(0);
    StorageRead::Bytes(bytes)
}

/// Named to the client in the next snapshot, for it to set aside.
fn refuse(file: StorageFile) -> StorageRead {
    refusals()[file as usize] = true;
    request_world_tick();
    StorageRead::Unreadable
}

/// The file `file` holds `bytes`, sealed with its slot's format.
fn seal(file: StorageFile, bytes: &[u8]) -> Vec<u8> {
    let mut whole = Vec::with_capacity(1 + bytes.len());
    whole.push(file.format());
    whole.extend_from_slice(bytes);
    whole
}

enum ReadState {
    /// The heap refused its room: opened again after the heap gives some
    /// back, never taken for a file that held nothing. No descriptor is kept.
    Refused(Refusal),
    /// The runtime's until its head has passed the read (`take_read`).
    Reading(Reading),
    Read(StorageRead),
}

const _: () = {
    let mut i = 0;
    while i < StorageFile::ALL.len() {
        assert!(StorageFile::ALL[i] as usize == i);
        i += 1;
    }
};

/// Each storage file's read, at its `StorageFile`; none where none is waiting.
static READS: Global<[Option<ReadState>; StorageFile::ALL.len()]> = Global::new();

fn read_state(file: StorageFile) -> &'static mut Option<ReadState> {
    &mut READS.get(|| [const { None }; StorageFile::ALL.len()])[file as usize]
}

fn set_read_state(file: StorageFile, read: ReadState) {
    *read_state(file) = Some(read);
    request_world_tick();
}

fn on_storage_opened(file: StorageFile, fd: Fd, opened: Opened) {
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
        } => ReadState::Read(refuse(file)),
        Opened::Failed { .. } => ReadState::Read(StorageRead::Unreadable),
    };
    set_read_state(file, read);
}

/// The client's answer to `open_storage`, for a test with no runtime: `bytes`,
/// read whole with the runtime's head past the read, or an open that came to no
/// bytes.
#[cfg(test)]
pub fn on_storage_answered(file: StorageFile, answer: Result<&[u8], Opened>) {
    match answer {
        Ok(bytes) => set_read_state(file, ReadState::Read(storage_of(file, bytes.to_vec()))),
        Err(opened) => on_storage_opened(file, 0, opened),
    }
}

/// What a storage file held, once it is read whole; None until then.
pub fn take_storage(file: StorageFile) -> Option<StorageRead> {
    let slot = read_state(file);
    match slot.take()? {
        ReadState::Refused(refused) if refused.over() => {
            open_storage(file);
            None
        }
        ReadState::Reading(reading) => match take_read(reading) {
            Ok(bytes) => Some(storage_of(file, bytes)),
            Err(reading) => {
                set_read_state(file, ReadState::Reading(reading));
                None
            }
        },
        ReadState::Read(read) => Some(read),
        refused @ ReadState::Refused(_) => {
            *slot = Some(refused);
            None
        }
    }
}

/// `dst` is filled once the runtime's head has passed the read: where it was
/// posted, for `is_consumed`. An open is answered only where there is a runtime.
///
/// # Safety
/// As `Door::read`: `dst` stays allocated, unmoved and unread until
/// `is_consumed` says the runtime is done with it.
pub unsafe fn read(fd: Fd, dst: &mut [u8]) -> Pos {
    let door = door().or_bug(Errno::ENOTRECOVERABLE);
    // SAFETY: the caller's.
    unsafe { door.read(fd, dst) }
}

/// Whether the runtime's head has passed the read posted at `pos`, so its
/// buffer is the core's again.
pub fn is_consumed(pos: Pos) -> bool {
    door().or_bug(Errno::ENOTRECOVERABLE).is_consumed(pos)
}

/// An opened file of `bytes`, read whole and closed (`Files::read_whole`):
/// one owner, so every file read whole waits on a refusal alike. Where the
/// heap refuses its room the read is the owner's to ask again once
/// `Refusal::over`; no descriptor is kept meanwhile. An open is answered only
/// where there is a runtime.
pub fn read_whole(fd: Fd, bytes: u32) -> Result<Reading, Refusal> {
    let door = door().or_bug(Errno::ENOTRECOVERABLE);
    files()
        .read_whole(door, fd, bytes)
        .map_err(|_| Refusal::now())
}

/// The bytes `reading` holds, once the runtime's head has passed its read; the
/// reading again before then.
pub fn take_read(reading: Reading) -> Result<Vec<u8>, Reading> {
    reading.take(door().or_bug(Errno::ENOTRECOVERABLE))
}

pub fn close(fd: Fd) {
    if let Some(door) = door() {
        files().close(door, fd);
    }
}

/// An object's path below its root, written where it is wanted, so no path is
/// allocated.
#[derive(Clone, Copy, PartialEq)]
pub struct Path {
    bytes: [u8; PATH_BYTES],
    len: usize,
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

    /// A file's own name.
    pub fn of_name(name: &str) -> Path {
        let mut p = Path::EMPTY;
        p.write_str(name).or_bug(Errno::EOVERFLOW);
        p
    }

    /// A tile's or a shard's key by its id, or the objects' top.
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
        core::str::from_utf8(&self.bytes[..self.len]).or_bug(Errno::ENOTRECOVERABLE)
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

/// The id `path` below OBJECT is (`Path::of_quadrant`); none for what is no
/// shard's and not the top.
pub fn id_of_path(path: &str) -> Option<u64> {
    if path == object::TOP {
        return Some(TOP_ID);
    }
    Quadrant::of_key(path).map(Quadrant::id)
}

pub fn find(owner: impl Fn(&Owner) -> bool) -> Option<Fd> {
    files().find(owner)
}

/// The turn `write` last wrote in, and how many files it wrote there.
static LAST_WRITE: Global<(u64, u32)> = Global::new();

/// Writes `bytes` whole as `root`'s object `path`, where the turn has a
/// write left (`WRITES_A_TURN`); false where it has none, nothing written,
/// for its owner to write on a later turn. Nothing is written, and none
/// counted, where there is no runtime.
pub fn write(root: u32, path: Path, bytes: Vec<u8>) -> bool {
    let Some(door) = door() else { return true };
    let written = LAST_WRITE.get(|| (0, 0));
    if written.0 != turn() {
        *written = (turn(), 0);
    }
    if written.1 >= WRITES_A_TURN {
        return false;
    }
    written.1 += 1;
    files().write_whole(door, root, path.as_str(), bytes, Owner::Write);
    true
}

/// Writes `bytes` whole as what `file` holds; a refused slot is not written.
pub fn write_storage(file: StorageFile, bytes: &[u8]) {
    if is_refused(file) {
        return;
    }
    if let Some(door) = door() {
        files().write_whole(
            door,
            root::STORAGE,
            file.path().as_str(),
            seal(file, bytes),
            Owner::Write,
        );
    }
}

/// An open the runtime completed, read once (`Opened::of`); xpute's table finds
/// its owner and lets its handle go where it failed. A file larger than the
/// heap every device is given cannot be held anywhere: it is the client's
/// answer refused, closed here, and the owner hears EFBIG.
pub fn on_opened(completed: &FsOpened) {
    let FsOpened { fd, detail, .. } = *completed;
    let Some(owner) = door().and_then(|_| files().complete_open(completed)) else {
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
        Owner::StorageFile(file) => on_storage_opened(file, fd, opened),
        Owner::Write => {}
    }
}

#[cfg(test)]
#[path = "file.test.rs"]
mod test;
