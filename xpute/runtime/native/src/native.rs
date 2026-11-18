// xpute-runtime-native/native.rs

//! The runtime as a native app reaches it, as @xpute/runtime-web is the
//! web's: xpute's module starts the guest and calls the runtime's entries
//! through the C ABI (xpute.h), which the program exports by `native_abi!`
//! with what it starts its guest with (`Boot`), and on Android the same calls
//! under JNI's names (jni.rs). What the app hears, a batch waiting, a stop
//! and a trap, it hears through the functions it sets, and a stop and a trap
//! told before its UI started it reads (listener.rs).

use std::path::PathBuf;
use std::sync::OnceLock;
use std::thread::ThreadId;

use crate::log::log;
use crate::{event_loop, listener, remote};
use xpute_host::abi::log::Level;
use xpute_host::runtime::{self, Runtime, Stop};
use xpute_kit::status::bug::{OrBug, XputeBug};
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::strerror;
use xpute_link::abi::types::Token;
use xpute_link::{Handle, Lift, Method, Side, load_uri};

use crate::link::{self, side};

/// What mounts the implementations of some of the guest's imports on the
/// host's side the runtime gives it, on the guest's thread.
pub type Mount = Box<dyn FnOnce(&'static Side) + Send>;

/// What a program starts its guest with: the stack its thread takes, its
/// mounts, in no order, since none depends on another; the interfaces its
/// guest's world imports (the bindings' `IMPORTS`); and the methods they
/// hold that answer at once, xpute's and a plugin's among them (the
/// bindings' `CALLS`), which the UI may not implement.
pub struct Boot {
    pub stack_sz: usize,
    pub mounts: Vec<Mount>,
    pub imports: &'static [(&'static str, Handle)],
    pub calls: &'static [(Handle, Method)],
}

/// The program's start, which `native_abi!` takes: given the directory the
/// platform keeps the app's caches in, what it starts its guest with.
pub type Start = fn(PathBuf) -> Boot;

/// The prefixes the UI opens at, each with the named handle of the interface
/// whose resources they are, from a lowered `list<tuple<u32, list<token>>>`.
fn prefixes(pkt: &[u8]) -> Result<Vec<(Handle, Vec<Token>)>, Errno> {
    let r = Lift::new(pkt);
    let (base, len) = r.list(0, 12, 4)?;
    (0..len)
        .map(|k| Ok((r.u32(base + k * 12)?, load_uri(r, base + k * 12 + 4)?)))
        .collect()
}

/// Whether the runtime has started, which it does once a process.
static STARTED: std::sync::Once = std::sync::Once::new();

/// Starts the program's runtime (`xpute_start`): `named` and `prefixes` the
/// UI's, and the caches' directory. The runtime starts once a process
/// (`run`), and each call attaches a new UI to it (`remote::accept`), as a
/// reload's, which implements what the first did, and whose attach the guest
/// is told. It answers the new UI's handle, which the UI's posts, answers and
/// takes name.
pub fn start(start: Start, named: Vec<Handle>, prefixes_pkt: &[u8], cache_dir: PathBuf) -> u32 {
    let prefixes = prefixes(prefixes_pkt).or_bug(Errno::EBADMSG);
    let ui = remote::accept();
    STARTED.call_once(|| run(start(cache_dir), named, prefixes));
    ui
}

/// The guest's thread, the one that mounts the host's implementations and
/// enters the guest, which the runtime names as it starts it (`run`).
static GUEST_THREAD: OnceLock<ThreadId> = OnceLock::new();

/// Whether this is the guest's thread, where the link's state is.
fn on_guest_thread() -> bool {
    GUEST_THREAD.get() == Some(&std::thread::current().id())
}

/// Starts the guest on a thread of its own, as @xpute/runtime-web worker.ts
/// `run` does on the web: names the thread the guest's, gives the host what it
/// needs of the runtime (xpute-host runtime.rs `Runtime`), and once a failure
/// of the host's is a trap (`hook`), runs each mount on the host's side it
/// makes (`link::side`), then mounts what the UI implements
/// (`remote::mount`), enters the guest, which an import with no
/// implementation and a function without `async` the UI implements refuse
/// (`link::init`), and runs the loop for the guest's life. The guest is told
/// each UI attached by `remote::accept`. A failure of the host's anywhere on
/// this thread traps the link and ends only the step it was in, boot's or the
/// loop's turn: a trapped guest is entered no more, and the UI's methods are
/// answered EPIPE; what boot made due or completed is not left waiting.
fn run(boot: Boot, named: Vec<Handle>, prefixes: Vec<(Handle, Vec<Token>)>) {
    let spawned = std::thread::Builder::new()
        .name("xpute guest".into())
        .stack_size(boot.stack_sz)
        .spawn(move || {
            GUEST_THREAD.get_or_init(|| std::thread::current().id());
            runtime::init(Runtime {
                post: event_loop::post_task,
                after: event_loop::after,
                clear: event_loop::clear,
                stopped,
                log,
            });
            hook();
            caught(|| {
                for m in boot.mounts {
                    m(side());
                }
                let over: Vec<Handle> = named
                    .iter()
                    .chain(prefixes.iter().map(|(h, _)| h))
                    .copied()
                    .filter(|h| boot.calls.iter().any(|(c, _)| c == h))
                    .collect();
                remote::mount(&named, prefixes);
                link::init(boot.imports, &over);
            });
            loop {
                caught(|| {
                    side().run_due();
                    remote::flush();
                    event_loop::turn();
                });
            }
        });
    spawned.or_bug(Errno::EAGAIN);
}

/// Writes the stop `s` of `context`, the guest or the host, with its message
/// where it has one, then tells the app (`listener::tell_stop`).
fn tell_stop(context: &str, s: Stop) {
    let msg = s.msg.as_deref().map_or(String::new(), |m| format!(": {m}"));
    log(
        Level::Error,
        context,
        &format!(
            "stopped: {} at {}:{} ({}, {}){msg}",
            strerror(s.errnum),
            s.file,
            s.line,
            s.n1,
            s.n2
        ),
    );
    listener::tell_stop(s);
}

/// The guest's stop, as its host hands it (xpute-host stop.rs).
fn stopped(s: Stop) {
    tell_stop("guest", s);
}

/// Runs `f`, a step of the guest's thread, a panic in it ending the step
/// alone, which `hook` has made a trap.
fn caught(f: impl FnOnce()) {
    let _ = std::panic::catch_unwind(core::panic::AssertUnwindSafe(f));
}

/// Makes a panic of the host's own, on any thread, a stop, as @xpute/runtime-web
/// worker.ts `hook` makes a throw one: where the guest has not trapped
/// already, the failure is written and told the app as a stop, with its errno,
/// a bug's or ENOTRECOVERABLE, where it was raised and its message, a bug's
/// or a panic's own; then the link is trapped, so that no answer the dead
/// thread owed leaves the guest waiting, and the app hears the trap after the
/// stop, its cause what the panic gives, its message and place, as the web's
/// trap carries the throw's, or a bug's place. A panic after the guest
/// trapped is no failure of the host's. The link's state is the guest
/// thread's: a panic there is told at once, before it unwinds,
/// and one on another thread is posted to it. A guest installs its own hook
/// over this one and hands back what is not its own (xpute-guest link.rs
/// `in_guest`). A bug panics with itself, which no report outside the guest
/// took.
fn hook() {
    std::panic::set_hook(Box::new(|info| {
        let payload = info.payload();
        let (stop, cause) = match payload.downcast_ref::<XputeBug>() {
            Some(b) => (
                Stop {
                    errnum: b.errnum as i32,
                    file: b.file.to_owned(),
                    line: b.line,
                    n1: b.a,
                    n2: b.b,
                    msg: b.msg.map(str::to_owned),
                },
                format!("{} at {}:{}", link::HOST_STOPPED, b.file, b.line),
            ),
            None => {
                let (file, line) = info.location().map_or(("", 0), |at| (at.file(), at.line()));
                let msg = payload
                    .downcast_ref::<&str>()
                    .map(|m| (*m).to_owned())
                    .or_else(|| payload.downcast_ref::<String>().cloned());
                (
                    Stop {
                        errnum: Errno::ENOTRECOVERABLE as i32,
                        file: file.to_owned(),
                        line,
                        n1: 0,
                        n2: 0,
                        msg,
                    },
                    format!("{}: {info}", link::HOST_STOPPED),
                )
            }
        };
        let fail = move || {
            if !side().trapped() {
                tell_stop("host", stop);
            }
            link::trap(&cause);
        };
        if on_guest_thread() {
            fail();
        } else {
            event_loop::post_task(Box::new(fail));
        }
    }));
}

/// The text of the `len` bytes of UTF-8 at `at`.
///
/// # Safety
/// `at` is `len` readable bytes.
pub unsafe fn text(at: *const u8, len: usize) -> String {
    // SAFETY: the caller's.
    String::from_utf8_lossy(unsafe { core::slice::from_raw_parts(at, len) }).into_owned()
}

/// A directory the app names by the `len` bytes of UTF-8 at `at`.
///
/// # Safety
/// `at` is `len` readable bytes.
pub unsafe fn dir(at: *const u8, len: usize) -> PathBuf {
    // SAFETY: the caller's.
    PathBuf::from(unsafe { text(at, len) })
}

/// `len` bytes at `at`, or none where `at` is null.
///
/// # Safety
/// `at` is null or `len` readable bytes.
pub unsafe fn bytes(at: *const u8, len: usize) -> Option<Vec<u8>> {
    // SAFETY: the caller's.
    (!at.is_null()).then(|| unsafe { core::slice::from_raw_parts(at, len) }.to_vec())
}

/// Hands over the batch the UI of the handle `ui` has not taken, the
/// caller's until `taken_free`.
pub fn take(ui: u32) -> (*mut u8, usize) {
    let b = Box::into_raw(remote::take(ui).into_boxed_slice());
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

/// Exports the runtime under xpute's names (xpute.h) for the
/// program's `$start` ([`Start`]), and on Android
/// under JNI's for the Kotlin class `xpute.Runtime` (`jni_abi!`).
#[macro_export]
macro_rules! native_abi {
    ($start:path) => {
        /// # Safety
        /// `named` is `named_len` handles, `prefixes` `prefixes_len` bytes, and
        /// `cache_dir` `cache_dir_len` bytes.
        #[unsafe(no_mangle)]
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

        #[unsafe(no_mangle)]
        pub extern "C" fn xpute_notify_on(f: Option<$crate::listener::Tell>) {
            $crate::listener::notify_on(f);
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn xpute_stopped_on(f: Option<$crate::listener::Stopped>) {
            $crate::listener::stopped_on(f);
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn xpute_trap_on(f: Option<$crate::listener::Trapped>) {
            $crate::listener::trap_on(f);
        }

        /// # Safety
        /// Each is a writable word.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn xpute_stop(
            errnum: *mut i32,
            file_len: *mut usize,
            line: *mut u32,
            n1: *mut u64,
            n2: *mut u64,
        ) -> *const u8 {
            let Some(s) = $crate::listener::stop() else {
                return ::core::ptr::null();
            };
            // SAFETY: the caller's words.
            unsafe {
                (*errnum, *file_len, *line, *n1, *n2) =
                    (s.errnum, s.file.len(), s.line, s.n1, s.n2);
            }
            s.file.as_ptr()
        }

        /// # Safety
        /// `len` is a writable word.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn xpute_trap(len: *mut usize) -> *const u8 {
            let Some(err) = $crate::listener::trap() else {
                return ::core::ptr::null();
            };
            // SAFETY: the caller's word.
            unsafe { *len = err.len() };
            err.as_ptr()
        }

        /// # Safety
        /// `pkt` is null or `len` readable bytes.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn xpute_post(
            ui: u32,
            tag: u32,
            h: u32,
            m: u32,
            pkt: *const u8,
            len: usize,
        ) {
            // SAFETY: the caller's.
            let pkt = unsafe { $crate::native::bytes(pkt, len) };
            $crate::post(ui, tag, h, m, pkt);
        }

        /// # Safety
        /// `data` is null or `len` readable bytes.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn xpute_answer(
            ui: u32,
            tag: u32,
            result: i32,
            data: *const u8,
            len: usize,
        ) {
            // SAFETY: the caller's.
            let data = unsafe { $crate::native::bytes(data, len) };
            $crate::answer(ui, tag, result, data);
        }

        /// # Safety
        /// `len` is a writable word.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn xpute_take(ui: u32, len: *mut usize) -> *mut u8 {
            let (at, n) = $crate::native::take(ui);
            // SAFETY: the caller's word.
            unsafe { *len = n };
            at
        }

        /// # Safety
        /// `at` and `len` are one `xpute_take`'s, freed once.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn xpute_taken_free(at: *mut u8, len: usize) {
            // SAFETY: the caller's.
            unsafe { $crate::native::taken_free(at, len) };
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn xpute_life(shown: i32) {
            $crate::life(shown != 0);
        }

        /// # Safety
        /// `context` is `context_len` readable bytes, and `msg`
        /// `msg_len`.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn xpute_log(
            level: u32,
            context: *const u8,
            context_len: usize,
            msg: *const u8,
            msg_len: usize,
        ) {
            // SAFETY: the caller's.
            let (context, msg) = unsafe {
                (
                    $crate::native::text(context, context_len),
                    $crate::native::text(msg, msg_len),
                )
            };
            $crate::log(level, &context, &msg);
        }

        #[cfg(target_os = "android")]
        $crate::jni_abi!($start);
    };
}
