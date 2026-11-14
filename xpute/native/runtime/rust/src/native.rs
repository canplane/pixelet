// xpute-native-runtime/native.rs

//! The runtime as a native device's app reaches it, as @xpute/web-runtime is
//! a page's: xpute's module starts the guest and calls the runtime's entries
//! through the C ABI (xpute.h), which the program exports by `native_abi!`
//! with what it starts its guest with (`Boot`), and on Android the same calls
//! under JNI's names (jni.rs). What the app hears, a batch waiting, a stop
//! and a trap, it hears through the functions it sets (`notify_on`,
//! `stopped_on`, `trap_on`).

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

use xpute_kit::status::bug::{Bug, OrBug};
use xpute_kit::status::errno::Errno;
use xpute_link::abi::types::Token;
use xpute_link::{load_uri, mem, Handle, Lift, Method};
use crate::{event_loop, link, remote};

/// The report of a broken invariant before the guest stops: the guest's
/// (wit/stop/stop.wit), as the host that serves the stop hands it to the app
/// (xpute-native-host stop.rs), or the host's own failure, which the runtime
/// tells (`hook`).
pub struct Stop {
    pub errnum: i32,
    pub file: String,
    pub line: u32,
    pub n1: u64,
    pub n2: u64,
}

/// What serves the guest some of its imports: it mounts them on the link,
/// on the guest's thread.
pub type Server = Box<dyn FnOnce() + Send>;

/// What a program starts its guest with: the stack its thread takes, its
/// servers, in no order, since none depends on another; the interfaces its
/// guest's world imports (the bindings' `IMPORTS`);
/// and the methods of the UI's named interfaces that answer later (the
/// bindings' `LATER`).
pub struct Boot {
    pub stack_sz: usize,
    pub servers: Vec<Server>,
    pub imports: &'static [(&'static str, Handle)],
    pub later: &'static [(Handle, Method)],
}

/// The program's start, which `native_abi!` takes: given the directory the
/// platform keeps the app's caches in and what tells the app a stop, what
/// it starts its guest with.
pub type Start = fn(PathBuf, fn(Stop)) -> Boot;

/// What tells the app a batch waits.
pub type Tell = extern "C" fn();

/// What tells the app the guest stopped: its errno, its file's UTF-8 and
/// length, its line and its two numbers.
pub type Stopped = extern "C" fn(i32, *const u8, usize, u32, u64, u64);

/// What tells the app the guest trapped: what stopped it, its UTF-8 and
/// length, as a page's trap carries its error (@xpute/web-runtime `Trap`).
pub type Trapped = extern "C" fn(*const u8, usize);

static NOTIFY: Mutex<Option<Tell>> = Mutex::new(None);
static STOPPED: Mutex<Option<Stopped>> = Mutex::new(None);
static TRAP: Mutex<Option<Trapped>> = Mutex::new(None);

pub fn notify_on(f: Option<Tell>) {
    *NOTIFY.lock().unwrap_or_else(|e| e.into_inner()) = f;
}

pub fn stopped_on(f: Option<Stopped>) {
    *STOPPED.lock().unwrap_or_else(|e| e.into_inner()) = f;
}

pub fn trap_on(f: Option<Trapped>) {
    *TRAP.lock().unwrap_or_else(|e| e.into_inner()) = f;
}

fn notify() {
    if let Some(f) = *NOTIFY.lock().unwrap_or_else(|e| e.into_inner()) {
        f();
    }
}

/// What the app was told of the guest's end: the last stop reported, as the
/// UI keeps it, and what trapped the guest, told again to each new UI
/// (`start`).
struct End {
    stop: Option<Stop>,
    trap: Option<String>,
}

static END: Mutex<End> = Mutex::new(End {
    stop: None,
    trap: None,
});

fn end() -> MutexGuard<'static, End> {
    END.lock().unwrap_or_else(|e| e.into_inner())
}

fn tell_stop(s: &Stop) {
    if let Some(f) = *STOPPED.lock().unwrap_or_else(|e| e.into_inner()) {
        f(s.errnum, s.file.as_ptr(), s.file.len(), s.line, s.n1, s.n2);
    }
}

fn tell_trap(err: &str) {
    if let Some(f) = *TRAP.lock().unwrap_or_else(|e| e.into_inner()) {
        f(err.as_ptr(), err.len());
    }
}

fn stopped(s: Stop) {
    let mut end = end();
    tell_stop(&s);
    end.stop = Some(s);
}

/// Tells the app the guest trapped, with what stopped it (link.rs `trap`).
pub(crate) fn trap(err: &str) {
    let mut end = end();
    tell_trap(err);
    end.trap = Some(err.to_owned());
}

/// The prefixes the UI opens at, each with the named handle of the interface
/// whose resources they are, from a lowered `list<tuple<u32, list<token>>>`.
fn prefixes(pkt: &[u8]) -> Result<Vec<(Handle, Vec<Token>)>, Errno> {
    let r = Lift::new(pkt);
    let (base, len) = r.list(0, 12, 4)?;
    (0..len).map(|k| Ok((r.u32(base + k * 12)?, load_uri(r, base + k * 12 + 4)?))).collect()
}

/// Whether the runtime has started, which it does once a process.
static STARTED: std::sync::Once = std::sync::Once::new();

/// Starts the program's runtime (`xpute_start`): `named` and `prefixes` the
/// UI's, and the caches' directory. The runtime starts once a process
/// (`run`), and each call is a new UI on it (`remote::accept`), as a
/// reload's, which serves what the first did, whose attach the guest is
/// told, and which is told the guest's end where it came (`End`). It
/// answers the new UI's generation, which the UI's posts, answers and takes
/// carry.
pub fn start(start: Start, named: Vec<Handle>, prefixes_pkt: &[u8], cache_dir: PathBuf) -> u32 {
    let prefixes = prefixes(prefixes_pkt).or_bug(Errno::EBADMSG);
    let generation = remote::accept();
    {
        // A stop told again lands before a trap told meanwhile.
        let end = end();
        if let Some(s) = &end.stop {
            tell_stop(s);
        }
        if let Some(err) = &end.trap {
            tell_trap(err);
        }
    }
    STARTED.call_once(|| run(start(cache_dir, stopped), named, prefixes));
    generation
}

/// Starts the guest on a thread of its own, as @xpute/web-runtime worker.ts
/// `start` does on a page: once a failure of the host's is a trap (`hook`),
/// mounts each server, then what the UI serves (`remote::serve`), enters the
/// guest, which an import no server mounts refuses (`link::init`), and runs
/// the loop for the guest's life. The guest is told each UI attached by
/// `remote::accept`. A failure of the host's anywhere on this thread traps
/// the link and ends only the step it was in, boot's or the loop's turn: a
/// trapped guest is entered no more, and the UI's methods are answered
/// EPIPE; what boot made due or completed is not left waiting.
fn run(boot: Boot, named: Vec<Handle>, prefixes: Vec<(Handle, Vec<Token>)>) {
    let spawned = std::thread::Builder::new()
        .name("xpute guest".into())
        .stack_size(boot.stack_sz)
        .spawn(move || {
            link::record_guest_thread();
            hook();
            caught(|| {
                for s in boot.servers {
                    s();
                }
                remote::serve(&named, prefixes, boot.later);
                link::init(boot.imports);
            });
            loop {
                caught(|| {
                    link::run_due();
                    remote::flush(notify);
                    event_loop::turn();
                });
            }
        });
    spawned.or_bug(Errno::EAGAIN);
}

/// Runs `f`, a step of the guest's thread, a panic in it ending the step
/// alone, which `hook` has made a trap.
fn caught(f: impl FnOnce()) {
    let _ = std::panic::catch_unwind(core::panic::AssertUnwindSafe(f));
}

/// Makes a panic of the host's own, on any thread, a stop, as @xpute/web-runtime
/// worker.ts `hook` makes a throw one: the link is trapped, so that no answer
/// the dead thread owed leaves the guest waiting, and where it traps just now,
/// the failure is logged and told the app as a stop, with its errno, a bug's or
/// ENOTRECOVERABLE, and where it was raised, before the app hears the trap. A
/// panic after the guest trapped is no failure of the host's. The link's state
/// is the guest thread's: a panic there is told at once, before it unwinds,
/// and one on another thread is posted to it. A guest installs its own hook
/// over this one and hands back what is not its own (xpute-guest link.rs
/// `in_guest`). A bug panics with itself, which no report outside the guest
/// took.
fn hook() {
    std::panic::set_hook(Box::new(|info| {
        let (errnum, file, line) = match info.payload().downcast_ref::<Bug>() {
            Some(b) => (b.errnum as i32, b.file.to_owned(), b.line),
            None => {
                let (file, line) = info.location().map_or(("", 0), |at| (at.file(), at.line()));
                (Errno::ENOTRECOVERABLE as i32, file.to_owned(), line)
            }
        };
        let fail = move || {
            if !link::trapped() {
                crate::log(&format!("[host] stopped: {errnum} at {file}:{line}"));
                stopped(Stop {
                    errnum,
                    file: file.clone(),
                    line,
                    n1: 0,
                    n2: 0,
                });
            }
            link::trap(&format!("{} at {file}:{line}", link::HOST_STOPPED));
        };
        if link::on_guest_thread() {
            fail();
        } else {
            event_loop::post_task(Box::new(fail));
        }
    }));
}

/// A directory the app names by the `len` bytes of UTF-8 at `at`.
///
/// # Safety
/// `at` is `len` readable bytes.
pub unsafe fn dir(at: *const u8, len: usize) -> PathBuf {
    // SAFETY: the caller's.
    PathBuf::from(String::from_utf8_lossy(unsafe { core::slice::from_raw_parts(at, len) }).into_owned())
}

/// `len` bytes at `at`, or none where `at` is null.
///
/// # Safety
/// `at` is null or `len` readable bytes.
pub unsafe fn bytes(at: *const u8, len: usize) -> Option<Vec<u8>> {
    // SAFETY: the caller's.
    (!at.is_null()).then(|| unsafe { core::slice::from_raw_parts(at, len) }.to_vec())
}

/// Hands over the batch the UI of `generation` has not taken, the caller's
/// until `taken_free`.
pub fn take(generation: u32) -> (*mut u8, usize) {
    let b = Box::into_raw(remote::take(generation).into_boxed_slice());
    (b.cast::<u8>(), b.len())
}

/// Frees what `take` handed over.
///
/// # Safety
/// `at` and `len` are one `take`'s, freed once.
pub unsafe fn taken_free(at: *mut u8, len: usize) {
    // SAFETY: the boxed slice `take` made, as it made it.
    drop(unsafe { Box::from_raw(core::ptr::slice_from_raw_parts_mut(at, len)) });
}

/// The guest's memory: its first byte and its size.
pub fn mem() -> (*mut u8, u32) {
    let r = mem::region();
    (r.base, r.len)
}

/// Exports the runtime under xpute's names (xpute.h) for the
/// program's `$start` ([`Start`]), and on Android under JNI's for the Kotlin
/// class `xpute.Runtime` (`jni_abi!`).
#[macro_export]
macro_rules! native_abi {
    ($start:path) => {
        /// # Safety
        /// `named` is `named_len` handles, `prefixes` `prefixes_len` bytes, and
        /// `cache_dir` `cache_dir_len` bytes.
        #[no_mangle]
        pub unsafe extern "C" fn xpute_start(
            named: *const u32,
            named_len: usize,
            prefixes: *const u8,
            prefixes_len: usize,
            cache_dir: *const u8,
            cache_dir_len: usize,
        ) -> u32 {
            // SAFETY: the caller's.
            unsafe {
                let named = ::core::slice::from_raw_parts(named, named_len).to_vec();
                let prefixes = ::core::slice::from_raw_parts(prefixes, prefixes_len);
                let cache_dir = $crate::native::dir(cache_dir, cache_dir_len);
                $crate::native::start($start, named, prefixes, cache_dir)
            }
        }

        #[no_mangle]
        pub extern "C" fn xpute_notify_on(f: Option<$crate::native::Tell>) {
            $crate::native::notify_on(f);
        }

        #[no_mangle]
        pub extern "C" fn xpute_stopped_on(f: Option<$crate::native::Stopped>) {
            $crate::native::stopped_on(f);
        }

        #[no_mangle]
        pub extern "C" fn xpute_trap_on(f: Option<$crate::native::Trapped>) {
            $crate::native::trap_on(f);
        }

        /// # Safety
        /// `pkt` is null or `len` readable bytes.
        #[no_mangle]
        pub unsafe extern "C" fn xpute_post(generation: u32, tag: u32, h: u32, m: u32, pkt: *const u8, len: usize) {
            // SAFETY: the caller's.
            let pkt = unsafe { $crate::native::bytes(pkt, len) };
            $crate::post(generation, tag, h, m, pkt);
        }

        /// # Safety
        /// `data` is null or `len` readable bytes.
        #[no_mangle]
        pub unsafe extern "C" fn xpute_answer(generation: u32, tag: u32, result: i32, data: *const u8, len: usize) {
            // SAFETY: the caller's.
            let data = unsafe { $crate::native::bytes(data, len) };
            $crate::answer(generation, tag, result, data);
        }

        /// # Safety
        /// `len` is a writable word.
        #[no_mangle]
        pub unsafe extern "C" fn xpute_take(generation: u32, len: *mut usize) -> *mut u8 {
            let (at, n) = $crate::native::take(generation);
            // SAFETY: the caller's word.
            unsafe { *len = n };
            at
        }

        /// # Safety
        /// `at` and `len` are one `xpute_take`'s, freed once.
        #[no_mangle]
        pub unsafe extern "C" fn xpute_taken_free(at: *mut u8, len: usize) {
            // SAFETY: the caller's.
            unsafe { $crate::native::taken_free(at, len) };
        }

        /// # Safety
        /// `len` is null or a writable word.
        #[no_mangle]
        pub unsafe extern "C" fn xpute_mem(len: *mut u32) -> *mut u8 {
            let (at, n) = $crate::native::mem();
            if !len.is_null() {
                // SAFETY: the caller's word.
                unsafe { *len = n };
            }
            at
        }

        #[no_mangle]
        pub extern "C" fn xpute_life(shown: i32) {
            $crate::life(shown != 0);
        }

        #[cfg(target_os = "android")]
        $crate::jni_abi!($start);
    };
}
