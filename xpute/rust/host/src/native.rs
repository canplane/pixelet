// xpute-host/native.rs

//! The host for a guest linked into its own process: it reserves the core
//! memory (`reserve`), runs every call into the guest on the guest's thread,
//! where its `Global`s are sound (`thread`), hands each hostcall the guest
//! makes meanwhile back to the waiting caller (`answer_with`), and keeps the
//! guest's blocks in its memory (`Alloc`). One guest a process, since its base is one, so the
//! C ABI is xpute's own names with no program's prefix (`native_abi!`).

mod alloc;
mod reserve;
mod thread;

use core::ptr::NonNull;
use std::sync::OnceLock;
use std::time::Instant;

use xpute_guest::mem::heap::HeapStat;
use xpute_guest::mem::section::PAGE_BYTES;

pub use alloc::Alloc;
pub use thread::{answer_with, Answer};
pub use xpute_guest::entry::{run, BootInfo, Entry};

pub struct Config {
    /// Reserved once, never grown.
    pub mem_bytes: u32,
    /// `BootInfo::data_end`: the library's statics are the process's, so the
    /// end of the front the program keeps at the memory's bottom, 0 for none.
    pub data_end: u32,
    /// A whole number of pages; the heap runs to the memory's end.
    pub heap_at: u32,
    pub stack_bytes: usize,
    pub thread_name: &'static str,
    pub clock: fn() -> f64,
}

/// The only way to call into the booted guest.
#[derive(Clone, Copy)]
pub struct Guest {
    _booted: (),
}

/// Reserves the memory and starts the guest's thread, marked as the guest's
/// with its base set before this returns. Once a process.
pub fn boot(cfg: &Config) -> Guest {
    assert!(
        cfg.heap_at < cfg.mem_bytes,
        "the heap begins at {} of a {}-byte memory",
        cfg.heap_at,
        cfg.mem_bytes
    );
    assert!(
        cfg.heap_at.is_multiple_of(PAGE_BYTES),
        "the heap begins at {}, not on a {PAGE_BYTES}-byte page",
        cfg.heap_at
    );
    assert!(
        reserve::core().is_none(),
        "a guest is booted once a process"
    );
    let core = reserve::map(cfg.mem_bytes as usize, cfg.heap_at as usize);
    thread::start(cfg.thread_name, cfg.stack_bytes, move || {
        xpute_guest::global::mark_thread();
        // SAFETY: the region was reserved for the process's life.
        unsafe { xpute_guest::mem::set(core.at, core.bytes) };
    });
    Guest { _booted: () }
}

impl Guest {
    /// `f` on the guest's thread, the caller waiting, and answering on its
    /// own thread each hostcall `f` makes (`answer_with`). Refused on the
    /// guest's thread, inside `while_idle` and inside an answer, where it
    /// would wait for itself.
    pub fn run<R: Send>(self, f: impl FnOnce() -> R + Send) -> R {
        thread::run(f)
    }

    /// `f` on this thread while the guest's runs nothing. Refused where `run` is.
    pub fn while_idle<R>(self, f: impl FnOnce() -> R) -> R {
        thread::while_idle(f)
    }

    /// Read between turns, never as one reference over the whole: the
    /// host's own blocks lie in it too.
    pub fn mem(self) -> (NonNull<u8>, usize) {
        let c = reserve::core().expect("a Guest is booted");
        (
            NonNull::new(core::ptr::with_exposed_provenance_mut(c.at))
                .expect("a mapping is not at 0"),
            c.bytes,
        )
    }

    pub fn heap_stat(self) -> HeapStat {
        alloc::stat()
    }

    pub fn boot_info(self, cfg: &Config) -> BootInfo {
        BootInfo {
            given: u32::try_from(self.mem().1).expect("the memory is at most 4 GiB"),
            data_end: cfg.data_end,
            clock: cfg.clock,
            hostcall: thread::hostcall,
            heap_stat: alloc::stat,
        }
    }
}

/// The process's clock in milliseconds, from its first reading.
pub fn now() -> f64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64() * 1000.0
}

/// Boots the guest with `cfg` on the first call and exports its entry and
/// memory under xpute's names: `xpute_run(double quota_ms) -> double`, which
/// runs `entry` (`Entry`) on the guest's thread;
/// `xpute_mem(uint32_t *bytes) -> uint8_t *`, `bytes` null or written with
/// the memory's size; and `xpute_hostcall_on(int32_t (*answer)(uint32_t))`,
/// whose `answer`, or none for null, answers the guest's hostcalls
/// (`answer_with`). `guest()` is the program's for its own calls.
#[macro_export]
macro_rules! native_abi {
    ($cfg:expr, $entry:expr $(,)?) => {
        mod xpute_native {
            #[allow(unused_imports)]
            use super::*;

            const ENTRY: $crate::native::Entry = $entry;

            fn cfg() -> &'static $crate::native::Config {
                static CFG: ::std::sync::OnceLock<$crate::native::Config> =
                    ::std::sync::OnceLock::new();
                CFG.get_or_init(|| $cfg)
            }

            pub fn guest() -> $crate::native::Guest {
                static GUEST: ::std::sync::OnceLock<$crate::native::Guest> =
                    ::std::sync::OnceLock::new();
                *GUEST.get_or_init(|| $crate::native::boot(cfg()))
            }

            fn boot_info() -> $crate::native::BootInfo {
                guest().boot_info(cfg())
            }

            pub fn run(quota_ms: f64) -> f64 {
                guest().run(move || $crate::native::run(quota_ms, boot_info, ENTRY))
            }

            pub fn mem(bytes: Option<&mut u32>) -> *mut u8 {
                let (at, n) = guest().mem();
                if let Some(bytes) = bytes {
                    *bytes = u32::try_from(n).expect("the memory is at most 4 GiB");
                }
                at.as_ptr()
            }
        }

        pub use xpute_native::guest;

        #[no_mangle]
        pub extern "C" fn xpute_run(quota_ms: f64) -> f64 {
            xpute_native::run(quota_ms)
        }

        #[no_mangle]
        pub extern "C" fn xpute_hostcall_on(answer: Option<$crate::native::Answer>) {
            $crate::native::answer_with(answer);
        }

        /// # Safety
        /// `bytes` is null or points at a `u32` the caller owns.
        #[no_mangle]
        pub unsafe extern "C" fn xpute_mem(bytes: *mut u32) -> *mut u8 {
            // SAFETY: the caller's word, or none.
            xpute_native::mem(unsafe { bytes.as_mut() })
        }
    };
}
