// xpute-host/native.rs

//! The host for a guest linked into the host's own process: it reserves the
//! core memory (`reserve`), runs every call into the guest on the guest's own
//! thread, where its `Global`s are sound (`thread`), and keeps the guest's
//! blocks in its memory (`Alloc`). One guest a process, since its base is
//! one — one reserve, one core — so its C ABI is xpute's own names,
//! `xpute_intr` and `xpute_mem`, with no program's prefix (`native_abi!`).
//! Unlike the conformance guest's native form, whose host hands it a
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
pub use xpute_guest::entry::{BootInfo, Entry};

/// The guest's numbers, which the host boots it with.
pub struct Plan {
    /// Reserved once, never grown.
    pub memory_bytes: u32,
    /// Where the guest's own ranges may begin (`BootInfo::data_end`): the
    /// library's statics are the process's, so this is the end of whatever
    /// front the program keeps at the memory's bottom, 0 for none.
    pub data_end: u32,
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

    /// What the host hands the guest booted with `plan`: the memory it
    /// reserved, the plan's front and clock, and the allocator's reading.
    pub fn boot_info(self, plan: &Plan) -> BootInfo {
        BootInfo {
            given: self.memory().1 as u32,
            data_end: plan.data_end,
            clock: plan.clock,
            heap_stat: alloc::stat,
        }
    }
}

/// The process's clock in milliseconds, from its first reading.
pub fn now() -> f64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64() * 1000.0
}

/// Boots the guest with `plan` on the first call and exports its entry and
/// its memory under xpute's names: `xpute_intr(double quota_ms) -> double`,
/// which calls `entry` (`Entry`) on the guest's thread, and
/// `xpute_mem(uint32_t *bytes) -> uint8_t *`, `bytes` null or written with
/// the memory's size. `guest()` and `boot_info()` are the program's for its
/// own calls.
#[macro_export]
macro_rules! native_abi {
    ($plan:expr, $entry:expr $(,)?) => {
        pub fn guest() -> $crate::native::Guest {
            static GUEST: ::std::sync::OnceLock<$crate::native::Guest> = ::std::sync::OnceLock::new();
            *GUEST.get_or_init(|| $crate::native::boot(&$plan))
        }

        /// What the host hands the guest (xpute_guest::entry `BootInfo`).
        pub fn boot_info() -> $crate::native::BootInfo {
            guest().boot_info(&$plan)
        }

        #[no_mangle]
        pub extern "C" fn xpute_intr(quota_ms: f64) -> f64 {
            let entry: $crate::native::Entry = $entry;
            guest().run(move || entry(quota_ms, boot_info))
        }

        /// # Safety
        /// `bytes` is null or points at a `u32` the caller owns.
        #[no_mangle]
        pub unsafe extern "C" fn xpute_mem(bytes: *mut u32) -> *mut u8 {
            let (at, n) = guest().memory();
            // SAFETY: the caller's word, or none.
            if let Some(bytes) = unsafe { bytes.as_mut() } {
                *bytes = n as u32;
            }
            at.as_ptr()
        }
    };
}
