// xpute-host/native.rs

//! The host for a guest linked into the host's own process: it reserves the
//! core memory (`reserve`), runs every call into the guest on the guest's own
//! thread, where its `Global`s are sound (`thread`), and keeps the guest's
//! blocks in its memory (`Alloc`). One guest a process, since its base is
//! one. Unlike the conformance guest's native form, whose host hands it a
//! memory, here the memory is the process's to reserve.

mod alloc;
mod reserve;
mod thread;

use core::ptr::NonNull;
use std::sync::OnceLock;
use std::time::Instant;

use xpute_guest::mem::heap::HeapStat;
use xpute_guest::mem::section::PAGE_BYTES;

pub use alloc::Alloc;

/// The guest's numbers, which the host boots it with.
pub struct Plan {
    /// Reserved once, never grown.
    pub memory_bytes: u32,
    /// The heap's offset, a whole number of pages; it runs to the memory's end.
    pub heap_at: u32,
    pub stack_bytes: usize,
    pub thread_name: &'static str,
    /// Installed on the guest's thread before its first call.
    pub clock: fn() -> f64,
}

/// The booted guest, the only way to call into it.
#[derive(Clone, Copy)]
pub struct Guest {
    _booted: (),
}

/// Reserves the memory and starts the guest's thread, marked as the guest's
/// with its base and clock set before this returns. Once a process.
pub fn boot(plan: &Plan) -> Guest {
    assert!(plan.heap_at < plan.memory_bytes, "the heap begins at {} of a {}-byte memory", plan.heap_at, plan.memory_bytes);
    assert!(plan.heap_at.is_multiple_of(PAGE_BYTES), "the heap begins at {}, not on a {PAGE_BYTES}-byte page", plan.heap_at);
    assert!(reserve::core().is_none(), "a guest is booted once a process");
    let core = reserve::reserve(plan.memory_bytes as usize, plan.heap_at as usize);
    let clock = plan.clock;
    thread::start(plan.thread_name, plan.stack_bytes, move || {
        xpute_guest::global::mark_thread();
        // SAFETY: the region was reserved for the process's life.
        unsafe { xpute_guest::mem::set(core.at, core.bytes) };
        xpute_guest::clock::set_clock(clock);
    });
    Guest { _booted: () }
}

impl Guest {
    /// `f` on the guest's thread, the caller waiting. Refused on the guest's
    /// thread and inside `while_idle`, where it would wait for itself.
    pub fn run<R: Send>(self, f: impl FnOnce() -> R + Send) -> R {
        thread::run(f)
    }

    /// `f` on this thread while the guest's runs nothing. Refused where `run` is.
    pub fn while_idle<R>(self, f: impl FnOnce() -> R) -> R {
        thread::while_idle(f)
    }

    /// The core memory's first byte and bytes, read between turns and never
    /// as one reference over the whole: the host's own blocks lie in it too.
    pub fn memory(self) -> (NonNull<u8>, usize) {
        let c = reserve::core().expect("a Guest is booted");
        (NonNull::new(core::ptr::with_exposed_provenance_mut(c.at)).expect("a mapping is not at 0"), c.bytes)
    }

    pub fn heap_stat(self) -> HeapStat {
        alloc::stat()
    }
}

/// The process's clock in milliseconds, from its first reading.
pub fn now() -> f64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64() * 1000.0
}

/// Boots the guest with `plan` on the first call and exports its turn and its
/// memory under the program's prefix: `<prefix>_interrupt(double quota_ms) ->
/// double` and `<prefix>_memory(uint32_t *bytes) -> uint8_t *`, `bytes` null
/// or written with the memory's size. `guest()` is the program's for its own calls.
#[macro_export]
macro_rules! native_abi {
    ($prefix:literal, $plan:expr, $entry:expr $(,)?) => {
        pub fn guest() -> $crate::native::Guest {
            static GUEST: ::std::sync::OnceLock<$crate::native::Guest> = ::std::sync::OnceLock::new();
            *GUEST.get_or_init(|| $crate::native::boot(&$plan))
        }

        #[unsafe(export_name = concat!($prefix, "_interrupt"))]
        pub extern "C" fn interrupt(quota_ms: f64) -> f64 {
            guest().run(move || ($entry)(quota_ms))
        }

        /// # Safety
        /// `bytes` is null or points at a `u32` the caller owns.
        #[unsafe(export_name = concat!($prefix, "_memory"))]
        pub unsafe extern "C" fn memory(bytes: *mut u32) -> *mut u8 {
            let (at, n) = guest().memory();
            // SAFETY: the caller's word, or none.
            if let Some(bytes) = unsafe { bytes.as_mut() } {
                *bytes = n as u32;
            }
            at.as_ptr()
        }
    };
}
