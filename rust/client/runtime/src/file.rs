// pxl-runtime/file.rs

//! A device's settings file (xpute wit/fs/file.wit), the one path the
//! runtime serves itself, on the device's disk (xpute-native-host `disk`),
//! as a page's are served (ts/client/src/service/sys/fs/file.ts): its bytes
//! read whole as it opens and waiting for its first method, a size or a read
//! answering ECANCELED where its caller cancels it or its file closes first,
//! what is written committed at its close, and a set-aside renaming it beside
//! itself. Every other path is the UI's, which serves it over the remote as
//! the page's worker does, its opener mounted at a shorter prefix.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

use pxl_core::spec::file::path;
use xpute_kit::status::errno::Errno;
use xpute_link::abi::types::{Lent, Token};
use xpute_link::{mem, Return};
use xpute_native_host::disk;
use xpute_native_runtime::abi::fs::file;
use xpute_native_runtime::abi::fs::types::ErrorCode;
use xpute_native_runtime::link;

/// What waits on a file's bytes.
type Waiter = Box<dyn FnOnce(&Result<Rc<[u8]>, Errno>)>;

/// A file's bytes, while they come and once they are in, shared by each
/// method that reads them.
enum Bytes {
    Coming(Vec<Waiter>),
    In(Result<Rc<[u8]>, Errno>),
}

struct State {
    bytes: Bytes,
    /// How far the reads have gone.
    read: usize,
    written: Vec<u8>,
}

struct File {
    /// Where its bytes are on the disk.
    path: Result<PathBuf, Errno>,
    state: Rc<RefCell<State>>,
}

/// Answers `task_return` with what `f` makes of the file's bytes once they
/// are in, or ECANCELED where its caller cancels it first, as a page's method
/// races its bytes against its caller's abort: whichever comes first answers.
fn when<T: 'static>(
    state: &Rc<RefCell<State>>,
    task_return: Return<T>,
    f: impl FnOnce(&Result<Rc<[u8]>, Errno>) -> Result<T, Errno> + 'static,
) {
    let mut s = state.borrow_mut();
    match &mut s.bytes {
        Bytes::In(b) => {
            let b = b.clone();
            drop(s);
            task_return.answer(f(&b));
        }
        Bytes::Coming(waiting) => {
            let pending = Rc::new(Cell::new(None::<Return<T>>));
            task_return.on_cancel({
                let pending = pending.clone();
                move || {
                    if let Some(r) = pending.take() {
                        r.answer(Err(Errno::ECANCELED));
                    }
                }
            });
            pending.set(Some(task_return));
            waiting.push(Box::new(move |b| {
                if let Some(r) = pending.take() {
                    r.answer(f(b));
                }
            }));
        }
    }
}

/// The bytes are in, or the file closed first: what waited on them runs. The
/// first comes to stay, so a read that lands after its file closed changes
/// nothing.
fn settle(state: &Rc<RefCell<State>>, b: Result<Rc<[u8]>, Errno>) {
    let waiting = {
        let mut s = state.borrow_mut();
        let Bytes::Coming(waiting) = &mut s.bytes else {
            return;
        };
        let waiting = core::mem::take(waiting);
        s.bytes = Bytes::In(b.clone());
        waiting
    };
    for f in waiting {
        f(&b);
    }
}

fn open(uri: &[Token]) -> File {
    let state = Rc::new(RefCell::new(State {
        bytes: Bytes::Coming(Vec::new()),
        read: 0,
        written: Vec::new(),
    }));
    let path = disk::path(uri);
    match &path {
        Ok(p) => {
            let s = state.clone();
            disk::read(p.clone(), move |b| settle(&s, b.map(Rc::from)));
        }
        Err(e) => settle(&state, Err(*e)),
    }
    File { path, state }
}

/// The fs answer of the disk's errno `e`: the file's error-code, or a
/// cancel, which is the link's.
fn answer<T>(e: Errno) -> Result<Result<T, ErrorCode>, Errno> {
    match e {
        Errno::ECANCELED => Err(e),
        Errno::ENOENT => Ok(Err(ErrorCode::NoEntry)),
        Errno::EACCES => Ok(Err(ErrorCode::Access)),
        Errno::EROFS => Ok(Err(ErrorCode::ReadOnly)),
        _ => Ok(Err(ErrorCode::Io)),
    }
}

impl file::HostFile for File {
    fn size(&self, task_return: Return<Result<u32, ErrorCode>>) {
        when(&self.state, task_return, |b| {
            b.as_ref()
                .map_or_else(|e| answer(*e), |b| Ok(Ok(b.len() as u32)))
        });
    }

    fn read(&self, buf: Lent, task_return: Return<Result<u32, ErrorCode>>) {
        let state = self.state.clone();
        when(&self.state, task_return, move |b| {
            let b = match b {
                Ok(b) => b,
                Err(e) => return answer(*e),
            };
            let mut s = state.borrow_mut();
            let n = (buf.len as usize).min(b.len() - s.read);
            // SAFETY: the bytes the core lends for the read, the callee's
            // until its answer.
            unsafe { mem::bytes(buf.at, n as u32) }.copy_from_slice(&b[s.read..s.read + n]);
            s.read += n;
            Ok(Ok(n as u32))
        });
    }

    fn write(&self, buf: Lent, task_return: Return<()>) {
        // SAFETY: the bytes the core lends for the write, until its answer.
        let bytes = unsafe { mem::read(buf.at, buf.len) };
        self.state.borrow_mut().written.extend_from_slice(bytes);
        task_return.answer(Ok(()));
    }

    fn set_aside(&self, task_return: Return<Result<(), ErrorCode>>) {
        match &self.path {
            Ok(p) => disk::set_aside(p.clone(), move |r| {
                task_return.answer(r.map_or_else(answer, |()| Ok(Ok(()))));
            }),
            Err(_) => task_return.answer(Ok(Err(ErrorCode::ReadOnly))),
        }
    }

    fn close(&self, task_return: Return<Result<(), ErrorCode>>) {
        settle(&self.state, Err(Errno::ECANCELED));
        let written = core::mem::take(&mut self.state.borrow_mut().written);
        if written.is_empty() {
            return task_return.answer(Ok(Ok(())));
        }
        match &self.path {
            Ok(p) => disk::commit(p.clone(), written, move |r| {
                task_return.answer(r.map_or_else(answer, |()| Ok(Ok(()))));
            }),
            Err(_) => task_return.answer(Ok(Err(ErrorCode::ReadOnly))),
        }
    }
}

/// Mounts the settings' opener, on the core's thread before it boots.
pub(crate) fn serve() {
    let settings = vec![Token::Int(path::storage::ID), Token::Int(path::storage::settings::ID)];
    link::mount(settings, |uri, _| Ok(file::file(Rc::new(open(uri)))));
}
