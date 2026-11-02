// pxl-kernel/sys/io/file.rs

//! xpute's file table with an owner per descriptor, so an open's answer
//! reaches its owner and one for a closed descriptor reaches no one. A
//! native test has no host and opens nothing.
//!
//! Every file is a root's object by number (host.json `root`): a tile by its
//! subject, an asset by its load, a storage file by its slot, a shard's
//! objects by the shard's subject, their top and their edits by theirs. The
//! number is the path's one name, in decimal; what it is and where, a
//! version, a URL or a storage key, is what the host
//! (ts/client/src/driver/namespace/) binds the root to, `@xpute/host`
//! carrying the open there. A file is bytes: how a platform keeps them, as
//! text or otherwise, is its backend's.

use xpute_guest::abi::cmd::SysOpened;
use xpute_guest::ipc::file::Files;
use xpute_guest::ipc::sys::Fd;

use crate::sys::io::failure::Failure;
use crate::sys::io::fetch;
use crate::sys::io::host::{answer_of, sys, turn};
use crate::sys::io::host_spec::{format, root, storage};
use crate::sys::io::loader;
use crate::sys::io::param::{MAX_CREDITS, WRITES_A_TURN};
use crate::sys::io::request::Credit;
use crate::sys::io::room::Refusal;
use crate::sys::mem::spec::MIN_HEAP_BYTES;
use crate::sys::sched::edge::request_world_tick;
use xpute_guest::global::Global;
use xpute_kit::status::errno::Errno;

/// A file the kernel keeps in storage, by its slot (host.json `storage`).
/// Storage is opened and written only by one of these, so what a turn can
/// record there follows from how many.
///
/// A storage file is its slot's format byte (host.json `format`), then what its
/// owner keeps. One of another format, one its backend cannot read as bytes
/// (EBADMSG), or one larger than any heap a device is given (no build reads
/// it) is refused: read as nothing and never written over, and named to the
/// host (`is_refused`), whose it is to set the file aside and empty the slot
/// (`on_emptied`). Which storage a slot is, how it keeps the bytes, and what
/// an older file becomes, the kernel never says.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StorageFile {
    Settings,
}

impl StorageFile {
    /// In declaration order, as `READS` is indexed by `kept as usize`.
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

    fn path(self) -> Path {
        Path::of(u64::from(self.slot()))
    }
}

/// The longest name the kernel opens: a number in decimal, a u64's widest,
/// so the host stream's reserve can count on it.
pub const PATH_BYTES: usize = 20;

const _: () = assert!(u64::MAX.ilog10() as usize + 1 == PATH_BYTES);

/// `Load` is an asset's file, read as a model or an image as its attempt's
/// subject says.
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

/// Opens `root`'s object `number`; none where there is no host.
pub fn open(_: Credit, root: u32, number: u64, owner: Owner) -> Option<Fd> {
    Some(files().open(sys()?, root, Path::of(number).as_str(), owner))
}

/// Opens a storage file to be read whole; what it held is taken with `take_storage`.
pub fn open_storage(file: StorageFile) -> Option<Fd> {
    Some(files().open(
        sys()?,
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
/// it was read, and nothing is written over it until the host empties the slot.
pub fn is_refused(file: StorageFile) -> bool {
    refusals()[file as usize]
}

/// The host set a refused slot's file aside and emptied it: its owner may
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

/// Named to the host in the next snapshot, for it to set aside.
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
    /// Filled from the turn after `asked`.
    Reading {
        bytes: Vec<u8>,
        asked: u64,
    },
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
        // SAFETY: the bytes move into READS, their heap buffer with them, and
        // are taken no sooner than the turn after this one.
        Opened::Bytes(n) => match unsafe { read_whole(fd, n) } {
            Ok(bytes) => ReadState::Reading {
                bytes,
                asked: turn(),
            },
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

/// The host's answer to `open_storage`, for a test with none: `bytes`, read
/// whole and taken from the next turn on, or an open that came to no bytes.
#[cfg(test)]
pub fn on_storage_answered(file: StorageFile, answer: Result<&[u8], Opened>) {
    match answer {
        Ok(bytes) => set_read_state(
            file,
            ReadState::Reading {
                bytes: bytes.to_vec(),
                asked: turn(),
            },
        ),
        Err(opened) => on_storage_opened(file, 0, opened),
    }
}

/// What a storage file held, once it is read whole; None until then.
pub fn take_storage(file: StorageFile) -> Option<StorageRead> {
    let slot = read_state(file);
    match slot.as_ref()? {
        &ReadState::Refused(refused) => {
            if refused.over() {
                *slot = None;
                open_storage(file);
            }
            None
        }
        ReadState::Reading { asked, .. } if turn() <= *asked => None,
        _ => match slot.take()? {
            ReadState::Reading { bytes, .. } => Some(storage_of(file, bytes)),
            ReadState::Read(file) => Some(file),
            ReadState::Refused(_) => None,
        },
    }
}

/// `dst` is filled from the next turn on.
///
/// # Safety
/// As `Sys::read`: `dst` outlives this turn, unmoved and unread.
pub unsafe fn read(fd: Fd, dst: &mut [u8]) {
    if let Some(sys) = sys() {
        // SAFETY: the caller's.
        unsafe { sys.read(fd, dst) };
    }
}

/// An opened file of `bytes`, read whole and closed: one owner, so every
/// file read whole waits on a refusal alike. Where the heap refuses its room
/// the read is the owner's to ask again once `Refusal::over`; no descriptor
/// is kept meanwhile.
///
/// # Safety
/// The bytes are filled from the next turn on: as `read`, they are neither
/// read nor dropped before then.
pub unsafe fn read_whole(fd: Fd, bytes: u32) -> Result<Vec<u8>, Refusal> {
    let mut held = Vec::new();
    let room = held.try_reserve_exact(bytes as usize);
    if room.is_ok() {
        held.resize(bytes as usize, 0);
        // SAFETY: the caller's.
        unsafe { read(fd, &mut held) };
    }
    close(fd);
    room.map(|()| held).map_err(|_| Refusal::now())
}

pub fn close(fd: Fd) {
    if let Some(sys) = sys() {
        files().close(sys, fd);
    }
}

/// An object's path below its root, its number in decimal, written where it
/// is wanted, so no path is allocated.
struct Path {
    digits: [u8; PATH_BYTES],
    from: usize,
}

impl Path {
    fn of(mut number: u64) -> Path {
        let mut p = Path {
            digits: [0; PATH_BYTES],
            from: PATH_BYTES,
        };
        loop {
            p.from -= 1;
            p.digits[p.from] = b'0' + (number % 10) as u8;
            number /= 10;
            if number == 0 {
                return p;
            }
        }
    }

    fn as_str(&self) -> &str {
        // SAFETY: ASCII digits only.
        unsafe { core::str::from_utf8_unchecked(&self.digits[self.from..]) }
    }
}

pub fn held_for(owner: impl Fn(&Owner) -> bool) -> Option<Fd> {
    files().held_for(owner)
}

/// The turn `write` last wrote in, and how many files it wrote there.
static LAST_WRITE: Global<(u64, u32)> = Global::new();

/// Writes `bytes` whole as `root`'s object `number`, where the turn has a
/// write left (`WRITES_A_TURN`); false where it has none, nothing written,
/// for its owner to write on a later turn. Nothing is written, and none
/// counted, where there is no host.
pub fn write(root: u32, number: u64, bytes: Vec<u8>) -> bool {
    let Some(sys) = sys() else { return true };
    let written = LAST_WRITE.get(|| (0, 0));
    if written.0 != turn() {
        *written = (turn(), 0);
    }
    if written.1 >= WRITES_A_TURN {
        return false;
    }
    written.1 += 1;
    files().write_whole(sys, root, Path::of(number).as_str(), bytes, Owner::Write);
    true
}

/// Writes `bytes` whole as what `file` holds; a refused slot is not written.
pub fn write_storage(file: StorageFile, bytes: &[u8]) {
    if is_refused(file) {
        return;
    }
    if let Some(sys) = sys() {
        files().write_whole(
            sys,
            root::STORAGE,
            file.path().as_str(),
            seal(file, bytes),
            Owner::Write,
        );
    }
}

/// What an open came to, read once from the host's answer.
#[derive(Clone, Copy, Debug)]
pub enum Opened {
    /// The file's size; the descriptor is the owner's to read and close.
    Bytes(u32),
    /// No such file; the descriptor is closed.
    Absent,
    /// Every other failure; the descriptor is closed.
    Failed { errnum: Errno, detail: u32 },
}

impl Opened {
    /// An open that came to no bytes, as its run's failure.
    #[track_caller]
    pub fn failure(self) -> Failure {
        match self {
            Opened::Failed { errnum, detail } => Failure {
                errnum,
                detail: detail as u16,
                at: None,
            },
            Opened::Absent => Failure::from(Errno::ENOENT),
            Opened::Bytes(_) => Failure::from(Errno::ENOTRECOVERABLE),
        }
    }
}

/// An open the host completed, its `res` the file's size or a negative
/// errno; xpute's table finds its owner and closes it where it failed. A file
/// larger than the heap every device is given cannot be held anywhere: it is
/// the host's answer refused, closed here, and the owner hears EFBIG.
pub fn on_opened(completed: &SysOpened) {
    let SysOpened { fd, res, detail } = *completed;
    let Some(owner) = sys().and_then(|sys| files().complete_open(sys, completed)) else {
        return;
    };
    let opened = match answer_of(res) {
        Ok(n) if n > MIN_HEAP_BYTES => {
            close(fd);
            Opened::Failed {
                errnum: Errno::EFBIG,
                detail,
            }
        }
        Ok(n) => Opened::Bytes(n),
        Err(Errno::ENOENT) => Opened::Absent,
        Err(errnum) => Opened::Failed { errnum, detail },
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
