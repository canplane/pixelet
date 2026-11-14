// pxl-core/sys/io/file.rs

//! The core's files, over xpute's fs (wit/fs/file.wit). A file is opened by its
//! path's URI to a `File`, which lets its handle go as it drops, and is read or
//! written whole by a task: its size, one read of it and its close, or one
//! write and its close, whose errno says whether the write was applied. A
//! file's path is a tuple of tokens, as long as its branch of file.json's
//! `path` tree takes: each token names a child of the node before it, and a
//! branch that holds data takes the tokens left, a tile or a shard by its
//! quadkey and an asset by its load's id. No path is kept: each is built where
//! it is opened or written (`path_of`); what is kept is what a path is made
//! from, an id or a slot. Everything past the path, such as a version, a URL or
//! a storage key, is the client's to decide (ts/client/src/service/sys/fs/). A
//! file is bytes, and how a platform keeps them is up to its backend.
//!
//! The storage file holds the settings, at (STORAGE, SETTINGS) (file.json
//! `path`). It is a format byte (file.json `format`) followed by what the
//! settings keep, and it is read whole and written whole. A file of another
//! format, one its backend cannot read as bytes, or one larger than any heap
//! is refused: it reads as nothing, it is never written over, and it is named
//! to the client. A refused file is watched until it changes, as the client
//! sets it aside, and is read again (`watch`).
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
use xpute_link::abi::types::Token;
use xpute_guest::link;
use xpute_link::{Handle, Lend};
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use crate::sys::io::demand::{Source, Stream};
use crate::spec;
use crate::spec::file::{format, path};
use crate::spec::manifest::mem;
use crate::sys::sched::{executor, invalidate};

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

/// An open file, its handle let go as it drops.
pub struct File(Handle);

impl File {
    pub fn open(path: &[Token]) -> Result<File, Errno> {
        link::open(path).map(File)
    }
}

impl Drop for File {
    fn drop(&mut self) {
        link::drop(self.0);
    }
}

/// The longest quadkey, a leaf's.
pub const KEY_BYTES: usize = LEAF_ZOOM as usize;

const _: () = assert!(spec::file::LEAF_ZOOM == LEAF_ZOOM);

/// The id that names the objects' top among the object stream's ids. Every
/// other id is a tile's, and no tile's id is 0.
pub const TOP_ID: u64 = 0;

/// A refused file, which the client sets aside: the storage file, or an
/// objects' file by its want's id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused {
    Storage,
    Objects(u64),
}

/// The refused files watched at once: the storage file, and the objects'
/// files a snapshot names, one for each tile of the ground's demand
/// (domain/layer/object/repository.rs `watch_named`). A watch that ends has
/// left its slot before its owner names the next (`take_changed`).
pub const WATCHED: usize = 1 + GROUND_DEMAND_TILE_CAP as usize;

#[derive(Default)]
struct Files {
    /// The files the request scheduler's attempts read, by the attempt's key,
    /// for a stop to close.
    attempts: BTreeMap<u64, Handle>,
    /// The storage file as it was read, until its owner takes it.
    storage_read: Option<StorageRead>,
    /// Whether the storage file has been refused since the client last
    /// emptied its slot.
    storage_refused: bool,
    /// The writes not yet landed, in the order sent, and whether the writer
    /// runs.
    writes: VecDeque<(Vec<Token>, Box<[u8]>)>,
    writing: bool,
    /// The refused files that changed, until their owners take them
    /// (`take_changed`).
    changed: VecDeque<Refused>,
}

static FILES: Global<Files> = Global::new();

fn files() -> &'static mut Files {
    FILES.get(Files::default)
}

/// Reads the file at `path` whole, at most `most` bytes (EFBIG past them),
/// into the buffer `buf` makes for its size; `buf`'s refusal is the read's.
/// An attempt's read, by its `key`, is closed by `stop`, which cancels it
/// (ECANCELED).
pub async fn read_whole<B: Lend>(
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
    let n = file::file_size(f.0).await?;
    if n > most {
        return Err(Errno::EFBIG);
    }
    let (buf, read) = file::file_read(f.0, buf(n)?).await;
    if read? != n {
        return Err(Errno::EIO);
    }
    file::file_close(f.0).await?;
    Ok(buf)
}

/// A heap buffer of `n` bytes, for a read.
pub fn heap(n: u32) -> Result<Vec<u8>, Errno> {
    Ok(vec![0u8; n as usize])
}

/// Closes the file the attempt `key` reads, which cancels its read; what its
/// reader hears of it afterwards is dropped.
pub fn stop(key: u64) {
    if let Some(h) = files().attempts.remove(&key) {
        drop(file::file_close(h));
    }
}

/// Replaces the file at `path` with `bytes`. Writes land in the order sent,
/// one at a time, by one writer task.
pub fn write(path: Vec<Token>, bytes: Box<[u8]>) {
    let f = files();
    f.writes.push_back((path, bytes));
    if !core::mem::replace(&mut f.writing, true) {
        executor::spawner().spawn(writer().or_bug(Errno::ENOSPC));
    }
}

/// Lands the writes queued, until none is left. A write that fails is not
/// tried again: what it held is written again by the next.
#[embassy_executor::task]
async fn writer() {
    while let Some((path, bytes)) = files().writes.pop_front() {
        let _ = write_whole(&path, bytes).await;
    }
    files().writing = false;
}

async fn write_whole(path: &[Token], bytes: Box<[u8]>) -> Result<(), Errno> {
    let f = File::open(path)?;
    let (_, written) = file::file_write(f.0, bytes.into_vec()).await;
    written?;
    file::file_close(f.0).await
}

/// The storage file's path, its slot after STORAGE.
pub fn storage_path() -> Vec<Token> {
    vec![
        Token::Int(path::storage::ID),
        Token::Int(path::storage::settings::ID),
    ]
}

/// Reads the storage file whole, for `take_storage`.
pub fn open_storage() {
    executor::spawner().spawn(storage_read().or_bug(Errno::ENOSPC));
}

#[embassy_executor::task]
async fn storage_read() {
    let read = match read_whole(&storage_path(), mem::MIN_HEAP_BYTES, None, heap).await {
        Ok(bytes) => storage_of(&bytes),
        // Only an absent file means nothing was kept.
        Err(Errno::ENOENT) => StorageRead::None,
        // The file is too large for any heap, or its backend cannot read it
        // as bytes.
        Err(Errno::EFBIG | Errno::EBADMSG) => refuse(),
        Err(_) => StorageRead::Unreadable,
    };
    files().storage_read = Some(read);
    invalidate();
}

/// What the storage file held, once it is read whole, or None until then.
pub fn take_storage() -> Option<StorageRead> {
    files().storage_read.take()
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
/// client, which sets the file aside, and the file is watched until it does.
fn refuse() -> StorageRead {
    files().storage_refused = true;
    watch(Refused::Storage);
    invalidate();
    StorageRead::Unreadable
}

pub fn is_storage_refused() -> bool {
    files().storage_refused
}

/// Writes `bytes` whole as the storage file's contents, after its format
/// byte. A refused file is not written over.
pub fn write_storage(bytes: &[u8]) {
    if is_storage_refused() {
        return;
    }
    write(storage_path(), [&[format::SETTINGS][..], bytes].concat().into_boxed_slice());
}

impl Refused {
    pub fn path(self) -> Vec<Token> {
        match self {
            Refused::Storage => storage_path(),
            Refused::Objects(id) => quadrant_path(path::object::ID, id),
        }
    }
}

/// Watches the refused file `r` until it changes under the core, as the
/// client sets it aside; then it is read again.
pub fn watch(r: Refused) {
    executor::spawner().spawn(watched(r).or_bug(Errno::ENOSPC));
}

#[embassy_executor::task(pool_size = WATCHED)]
async fn watched(r: Refused) {
    let changed = match File::open(&r.path()) {
        Ok(f) => file::file_changed(f.0).await,
        Err(e) => Err(e),
    };
    if changed.is_err() {
        return;
    }
    if let Refused::Storage = r {
        files().storage_refused = false;
    }
    files().changed.push_back(r);
    invalidate();
}

/// A refused file that has changed since, for its owner to read again: the
/// settings for the storage file, the objects' repository for theirs.
pub fn take_changed() -> Option<Refused> {
    files().changed.pop_front()
}

/// Text held inline, as a quadkey is written: the first `len` of `N` bytes,
/// zero past them.
#[derive(Clone, Copy, PartialEq)]
pub struct Text<const N: usize> {
    pub bytes: [u8; N],
    len: usize,
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

/// The path that `stream`'s want `id` opens, built at the length its branch
/// takes: a tile or a shard by its quadkey after its stream's token, the
/// objects' top as (OBJECT, TOP). A load opens through the loader
/// (sys/io/loader.rs), so its stream has no path here.
pub fn path_of(stream: Stream, id: u64) -> Option<Vec<Token>> {
    match stream.row().source {
        Source::Load => None,
        Source::Bake(_) | Source::Objects => Some(quadrant_path(stream.token()?, id)),
    }
}

/// The path of the tile or shard `id` after `head`: its quadkey, or, for
/// `TOP_ID`, the objects' top.
pub fn quadrant_path(head: i32, id: u64) -> Vec<Token> {
    if id == TOP_ID {
        return vec![Token::Int(head), Token::Int(path::object::top::ID)];
    }
    let mut key = Text::<KEY_BYTES>::EMPTY;
    Quadrant::of_id(id)
        .write_key(&mut key)
        .or_bug(Errno::EOVERFLOW);
    vec![Token::Int(head), Token::Str(key.as_str().to_owned())]
}
