// xpute-native-host/native.rs

//! The runtime and its host as a native device's app reaches them: xpute's
//! module calls the program's start and the runtime's entries through the C
//! ABI (xpute.h), which the program exports by `native_abi!` with its start,
//! and on Android the same calls under JNI's names (jni.rs). What the app
//! hears, a batch waiting, a stop and a trap, it hears through the functions
//! it sets (`notify_on`, `stopped_on`, `trap_on`).

use std::path::PathBuf;
use std::sync::Mutex;

use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;
use xpute_link::abi::types::Token;
use xpute_link::{load_uri, mem, Handle, Lift};
use xpute_native_runtime::remote;

use crate::stop::Stop;

/// The program's start, which `native_abi!` takes: the named handles of the
/// interfaces the UI serves, the prefixes it opens at, the directory the
/// platform's storage is kept under and the one it keeps the app's caches
/// in, and what tells the app a batch waits, a stop and a trap.
pub type Start = fn(Vec<Handle>, Vec<Vec<Token>>, PathBuf, PathBuf, fn(), fn(Stop), fn());

/// What tells the app a batch waits, or the guest's trap.
pub type Tell = extern "C" fn();

/// What tells the app the guest stopped: its errno, its file's UTF-8 and
/// length, its line and its two numbers.
pub type Stopped = extern "C" fn(i32, *const u8, usize, u32, u64, u64);

static NOTIFY: Mutex<Option<Tell>> = Mutex::new(None);
static STOPPED: Mutex<Option<Stopped>> = Mutex::new(None);
static TRAP: Mutex<Option<Tell>> = Mutex::new(None);

pub fn notify_on(f: Option<Tell>) {
    *NOTIFY.lock().unwrap_or_else(|e| e.into_inner()) = f;
}

pub fn stopped_on(f: Option<Stopped>) {
    *STOPPED.lock().unwrap_or_else(|e| e.into_inner()) = f;
}

pub fn trap_on(f: Option<Tell>) {
    *TRAP.lock().unwrap_or_else(|e| e.into_inner()) = f;
}

fn notify() {
    if let Some(f) = *NOTIFY.lock().unwrap_or_else(|e| e.into_inner()) {
        f();
    }
}

fn stopped(s: Stop) {
    if let Some(f) = *STOPPED.lock().unwrap_or_else(|e| e.into_inner()) {
        f(s.errnum, s.file.as_ptr(), s.file.len(), s.line, s.n1, s.n2);
    }
}

fn trap() {
    if let Some(f) = *TRAP.lock().unwrap_or_else(|e| e.into_inner()) {
        f();
    }
}

/// The prefixes the UI opens at, from a lowered `list<list<token>>`.
fn prefixes(pkt: &[u8]) -> Result<Vec<Vec<Token>>, Errno> {
    let r = Lift::new(pkt);
    let (base, len) = r.list(0, 8, 4)?;
    (0..len).map(|k| load_uri(r, base + k * 8)).collect()
}

/// Whether the runtime has started, which it does once a process.
static STARTED: std::sync::Once = std::sync::Once::new();

/// Starts the program's runtime (`xpute_start`): `named` and `prefixes` the
/// UI's, and the two directories. The runtime starts once a process, and
/// each call is a new UI on it (`remote::accept`), as a reload's, which
/// serves what the first did.
pub fn start(start: Start, named: Vec<Handle>, prefixes_pkt: &[u8], files_dir: PathBuf, cache_dir: PathBuf) {
    let prefixes = prefixes(prefixes_pkt).or_bug(Errno::EBADMSG);
    remote::accept();
    STARTED.call_once(|| start(named, prefixes, files_dir, cache_dir, notify, stopped, trap));
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

/// Hands over the batch the UI has not taken, the caller's until
/// `taken_free`.
pub fn take() -> (*mut u8, usize) {
    let b = Box::into_raw(remote::take().into_boxed_slice());
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

/// Exports the runtime and its host under xpute's names (xpute.h) for the
/// program's `$start` ([`Start`]), and on Android under JNI's for the Kotlin
/// class `xpute.Runtime` (`jni_abi!`).
#[macro_export]
macro_rules! native_abi {
    ($start:path) => {
        /// # Safety
        /// `named` is `named_len` handles, `prefixes` `prefixes_len` bytes, and
        /// each directory its length's bytes.
        #[no_mangle]
        pub unsafe extern "C" fn xpute_start(
            named: *const u32,
            named_len: usize,
            prefixes: *const u8,
            prefixes_len: usize,
            files_dir: *const u8,
            files_dir_len: usize,
            cache_dir: *const u8,
            cache_dir_len: usize,
        ) {
            // SAFETY: the caller's.
            unsafe {
                let named = ::core::slice::from_raw_parts(named, named_len).to_vec();
                let prefixes = ::core::slice::from_raw_parts(prefixes, prefixes_len);
                let (files_dir, cache_dir) =
                    ($crate::native::dir(files_dir, files_dir_len), $crate::native::dir(cache_dir, cache_dir_len));
                $crate::native::start($start, named, prefixes, files_dir, cache_dir);
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
        pub extern "C" fn xpute_trap_on(f: Option<$crate::native::Tell>) {
            $crate::native::trap_on(f);
        }

        /// # Safety
        /// `pkt` is null or `len` readable bytes.
        #[no_mangle]
        pub unsafe extern "C" fn xpute_post(tag: u32, h: u32, m: u32, pkt: *const u8, len: usize) {
            // SAFETY: the caller's.
            let pkt = unsafe { $crate::native::bytes(pkt, len) };
            ::xpute_native_runtime::post(tag, h, m, pkt);
        }

        /// # Safety
        /// `data` is null or `len` readable bytes.
        #[no_mangle]
        pub unsafe extern "C" fn xpute_answer(tag: u32, result: i32, data: *const u8, len: usize) {
            // SAFETY: the caller's.
            let data = unsafe { $crate::native::bytes(data, len) };
            ::xpute_native_runtime::answer(tag, result, data);
        }

        /// # Safety
        /// `len` is a writable word.
        #[no_mangle]
        pub unsafe extern "C" fn xpute_take(len: *mut usize) -> *mut u8 {
            let (at, n) = $crate::native::take();
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
            ::xpute_native_runtime::life(shown != 0);
        }

        #[cfg(target_os = "android")]
        $crate::jni_abi!($start);
    };
}
