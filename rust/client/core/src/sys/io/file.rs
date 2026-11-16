// pxl-core/sys/io/file.rs

//! The core's files, over xpute's fs (wit/fs/file.wit). A file is opened by its
//! path's URI to a `File`, which lets its handle go as it drops, and is read or
//! written whole by a task: its size, one read of it and its close, or one
//! write and its close, whose errno says whether the write was applied. A
//! file's path is a tuple of tokens, as long as its branch of file.json5's
//! `path` tree takes: each token names a child of the node before it, and a
//! branch that holds data takes the tokens left, a tile or a shard by its
//! quadkey and an asset by its load's id. No path is kept: each is built where
//! it is opened or written (`path_of`); what is kept is what a path is made
//! from, an id or a slot. Everything past the path, such as a version, a URL or
//! a storage key, is the client's to decide (ts/client/src/service/sys/fs/). A
//! file is bytes, and how a platform keeps them is up to its backend.
//!
//! The storage file holds the settings, at (STORAGE, SETTINGS) (file.json5
//! `path`). It is a format byte (file.json5 `format`) followed by what the
//! settings keep, and it is read whole and written whole. A file of another
//! format, one its backend cannot read as bytes, or one larger than any heap
//! is refused: it reads as nothing, it is never written over, and its owner
//! sets it aside (fs `set-aside`), after which the settings write it from
//! their next keep on (`set_aside`).
//!
//! A read lends its buffer, which its caller makes once the file's size is
//! known (`read_whole`): a body slot (sys/io/body.rs) or a heap buffer
//! (`heap`). Either lies in the core's memory, so its offset holds.

use core::fmt;
use std::collections::{BTreeMap, VecDeque};

use pxl_global::Global;
use pxl_spec::space::coord::zoom::LEAF_ZOOM;
use pxl_spec::space::layer::demand::GROUND_DEMAND_TILE_CAP;
use pxl_spec::space::layer::quadrant::Quadrant;
use xpute_guest::abi::fs::file;
use xpute_guest::abi::fs::types::ErrorCode;
use xpute_guest::link;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;
use xpute_link::abi::types::Token;
use xpute_link::{Handle, Lend};

use crate::spec;
use crate::spec::file::{format, path};
use crate::spec::manifest::mem;
use crate::sys::io::demand::{Source, Stream};
use crate::sys::io::failure::{Exit, Failure, exit_of};
use crate::sys::sched::{executor, invalidate};

pub(crate) enum StorageRead {
    /// Nothing was kept, or the file was empty.
    None,
    /// What the owner kept, without the format byte.
    Bytes(Vec<u8>),
    /// A file is there in a form the core refuses: larger than any heap, not
    /// bytes to its backend, or of another format. Its owner sets it aside
    /// (`set_aside`) and never writes over it.
    Refused,
    /// A file is there but its read failed. Its owner must never write over
    /// it.
    Unreadable,
}

/// An open file, its handle let go as it drops.
pub(crate) struct File(Handle);

impl File {
    pub(crate) fn open(path: &[Token]) -> Result<File, Errno> {
        link::open(path).map(File)
    }
}

impl Drop for File {
    fn drop(&mut self) {
        link::drop(self.0);
    }
}

/// The longest quadkey, a leaf's.
pub(crate) const KEY_SZ: usize = LEAF_ZOOM as usize;

const _: () = assert!(spec::file::LEAF_ZOOM == LEAF_ZOOM);

/// The id that names the objects' top among the object stream's ids. Every
/// other id is a tile's, and no tile's id is 0.
pub(crate) const TOP_ID: u64 = 0;

/// A refused file, which the core sets aside: the storage file, or an
/// objects' file by its want's id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Refused {
    Storage,
    Object(u64),
}

/// The refused files set aside at once: the storage file, and an objects'
/// file for each tile of the ground's demand, the most wanted at once
/// (domain/layer/object/repository.rs `WANT_CAP`).
pub(crate) const SET_ASIDE_CAP: usize = 1 + GROUND_DEMAND_TILE_CAP as usize;

#[derive(Default)]
struct Files {
    /// The files the request scheduler's attempts read, by the attempt's key,
    /// for a stop to close.
    attempts: BTreeMap<u64, Handle>,
    /// The storage file as it was read, until its owner takes it.
    storage_read: Option<StorageRead>,
    /// The storage file's contents not yet written, the latest alone, and
    /// whether the writer runs.
    storage_write: Option<Box<[u8]>>,
    writing: bool,
    /// The refused files set aside, until their owners take them
    /// (`take_set_aside`).
    set_aside: VecDeque<Refused>,
}

static FILES: Global<Files> = Global::new();

fn files() -> &'static mut Files {
    FILES.get(Files::default)
}

/// Reads the file at `path` whole, at most `most` bytes (EFBIG past them),
/// into the buffer `buf` makes for its size; `buf`'s refusal is the read's.
/// An attempt's read, by its `key`, is closed by `stop`, which cancels it
/// (ECANCELED).
pub(crate) async fn read_whole<B: Lend>(
    path: &[Token],
    most: u32,
    key: Option<u64>,
    buf: impl FnOnce(u32) -> Result<B, Errno>,
) -> Result<B, Errno> {
    let f = File::open(path)?;
    if let Some(k) = key {
        files().attempts.insert(k, f.0);
    }
    let read = read_open(&f, most, buf).await;
    if let Some(k) = key {
        files().attempts.remove(&k);
    }
    read
}

/// The open file `f`'s size, one read of that many bytes, and its close.
async fn read_open<B: Lend>(
    f: &File,
    most: u32,
    buf: impl FnOnce(u32) -> Result<B, Errno>,
) -> Result<B, Errno> {
    let n = errno(file::file_size(f.0).await)?;
    if n > most {
        return Err(Errno::EFBIG);
    }
    let (buf, read) = file::file_read(f.0, buf(n)?).await;
    if errno(read)? != n {
        return Err(Errno::EIO);
    }
    errno(file::file_close(f.0).await)?;
    Ok(buf)
}

/// What an fs method came to as one errno, the number the core counts a
/// read's failure by: the link's refusal, or the errno of the file's
/// `error-code`.
fn errno<T>(r: Result<Result<T, ErrorCode>, Errno>) -> Result<T, Errno> {
    r?.map_err(|e| match e {
        ErrorCode::NoEntry => Errno::ENOENT,
        ErrorCode::Io => Errno::EIO,
        ErrorCode::ReadOnly => Errno::EROFS,
        ErrorCode::Access => Errno::EACCES,
        ErrorCode::BadMessage => Errno::EBADMSG,
        ErrorCode::RemoteUnreachable => Errno::ENETUNREACH,
        ErrorCode::NoObject => Errno::ENXIO,
    })
}

/// What `attempt` comes to, made again each time it ends aborted
/// (`exit_of`), as a device's runtime cancels what a reload of its UI leaves
/// of the files the UI implements, the storage file among them. A page's worker
/// dies with its page, so none of its methods ends so there.
async fn redo<T, F: Future<Output = Result<T, Errno>>>(
    mut attempt: impl FnMut() -> F,
) -> Result<T, Errno> {
    loop {
        match attempt().await {
            Err(e) if exit_of(Failure::from(e)) == Exit::Aborted => continue,
            r => return r,
        }
    }
}

/// A heap buffer of `n` bytes, for a read.
pub(crate) fn heap(n: u32) -> Result<Vec<u8>, Errno> {
    Ok(vec![0u8; n as usize])
}

/// Closes the file the attempt `key` reads, which cancels its read; what its
/// reader hears of it afterwards is dropped.
pub(crate) fn stop(key: u64) {
    if let Some(h) = files().attempts.remove(&key) {
        drop(file::file_close(h));
    }
}

/// Lands the storage file's latest contents, until none waits, one write at
/// a time. A write a reload aborted is made again (`redo`); one that fails
/// otherwise is not: the next keep writes what it held.
#[embassy_executor::task]
async fn writer() {
    while let Some(bytes) = files().storage_write.take() {
        let _ = redo(|| write_whole(bytes.clone())).await;
    }
    files().writing = false;
}

async fn write_whole(bytes: Box<[u8]>) -> Result<(), Errno> {
    let f = File::open(&storage_path())?;
    let (_, written) = file::file_write(f.0, bytes.into_vec()).await;
    written?;
    errno(file::file_close(f.0).await)
}

/// The storage file's path, its slot after STORAGE.
fn storage_path() -> Vec<Token> {
    vec![
        Token::Int(path::storage::ID),
        Token::Int(path::storage::settings::ID),
    ]
}

/// Reads the storage file whole, for `take_storage`, again where a reload
/// aborted it (`redo`).
pub(crate) fn open_storage() {
    executor::spawner().spawn(storage_read().or_bug(Errno::ENOSPC));
}

#[embassy_executor::task]
async fn storage_read() {
    let path = storage_path();
    let read = match redo(|| read_whole(&path, mem::MIN_HEAP_SZ, None, heap)).await {
        Ok(bytes) => storage_of(&bytes),
        // Only an absent file means nothing was kept.
        Err(Errno::ENOENT) => StorageRead::None,
        // The file is too large for any heap, or its backend cannot read it
        // as bytes.
        Err(Errno::EFBIG | Errno::EBADMSG) => StorageRead::Refused,
        Err(_) => StorageRead::Unreadable,
    };
    files().storage_read = Some(read);
    invalidate();
}

/// What the storage file held, once it is read whole, or None until then.
pub(crate) fn take_storage() -> Option<StorageRead> {
    files().storage_read.take()
}

/// The storage file's bytes as its backend read them: the settings' bytes
/// where the format byte is theirs, refused where it is any other.
fn storage_of(bytes: &[u8]) -> StorageRead {
    match bytes.split_first() {
        None => StorageRead::None,
        Some((&format::SETTINGS, settings)) => StorageRead::Bytes(settings.to_vec()),
        Some(_) => StorageRead::Refused,
    }
}

/// Writes `bytes` whole as the storage file's contents, after its format
/// byte, in place of any write still waiting. Its owner writes only a file it
/// read (domain/setting.rs `writes`).
pub(crate) fn write_storage(bytes: &[u8]) {
    let f = files();
    f.storage_write = Some([&[format::SETTINGS][..], bytes].concat().into_boxed_slice());
    if !core::mem::replace(&mut f.writing, true) {
        executor::spawner().spawn(writer().or_bug(Errno::ENOSPC));
    }
}

impl Refused {
    fn path(self) -> Vec<Token> {
        match self {
            Refused::Storage => storage_path(),
            Refused::Object(id) => quadrant_path(vec![Token::Int(path::object::ID)], id),
        }
    }
}

/// Sets the refused file `r` aside, out of its path; then its owner takes it
/// back (`take_set_aside`): the settings write it from their next keep on,
/// the objects' repository reads it again. A set-aside that ended aborted is
/// made again (`redo`); one its backend cannot set aside, or one past the set-asides
/// that can wait at once, stays refused this session.
pub(crate) fn set_aside(r: Refused) {
    if let Ok(task) = setting_aside(r) {
        executor::spawner().spawn(task);
    }
}

#[embassy_executor::task(pool_size = SET_ASIDE_CAP)]
async fn setting_aside(r: Refused) {
    let set = redo(|| async {
        let f = File::open(&r.path())?;
        errno(file::file_set_aside(f.0).await)
    });
    if set.await.is_err() {
        return;
    }
    files().set_aside.push_back(r);
    invalidate();
}

/// A refused file set aside since, for its owner to take back: the settings
/// for the storage file, the objects' repository for theirs.
pub(crate) fn take_set_aside() -> Option<Refused> {
    files().set_aside.pop_front()
}

/// Text held inline, as a quadkey is written: the first `len` of `N` bytes,
/// zero past them.
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct Text<const N: usize> {
    pub bytes: [u8; N],
    len: usize,
}

impl<const N: usize> Text<N> {
    pub(crate) const EMPTY: Text<N> = Text {
        bytes: [0; N],
        len: 0,
    };

    pub(crate) fn as_str(&self) -> &str {
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

/// The path that `stream`'s want `id` opens (file.json5 `path`): a layer's
/// tile by its quadkey after the layer's node, numbered as the layer is
/// (`BASE`), a shard by its quadkey after OBJECT, the objects' top as
/// (OBJECT, TOP). A load opens through the loader (sys/io/loader.rs), so its
/// stream has no path here.
pub(crate) fn path_of(stream: Stream, id: u64) -> Option<Vec<Token>> {
    let head = match stream.row().source {
        Source::Load => return None,
        Source::Base(layer) => vec![Token::Int(layer as i32)],
        Source::Object => vec![Token::Int(path::object::ID)],
    };
    Some(quadrant_path(head, id))
}

/// The path of the tile or shard `id` after `head`: its quadkey, or, for
/// `TOP_ID`, the objects' top.
fn quadrant_path(mut head: Vec<Token>, id: u64) -> Vec<Token> {
    if id == TOP_ID {
        head.push(Token::Int(path::object::top::ID));
        return head;
    }
    let mut key = Text::<KEY_SZ>::EMPTY;
    Quadrant::of_id(id)
        .write_key(&mut key)
        .or_bug(Errno::EOVERFLOW);
    head.push(Token::Str(key.as_str().to_owned()));
    head
}
