// xpute-host/native.rs

//! The host's end for a guest linked into the host's own native process, as
//! `xpute_guest::module` is the guest's end of one built as a WebAssembly
//! module. The two share the process and its threads, so the host reserves
//! the core memory there itself (`reserve`), runs every call into the guest
//! on a thread of the guest's own (`thread`), and provides the allocator
//! that keeps the guest's blocks in its memory and every other thread's out
//! of it (`Alloc`).
//!
//! Not the form of a guest built as a native library that a host opens and
//! hands a memory it holds (the conformance guest's
//! `xpute_conformance_boot(mem, now)`): here the guest is part of the host's
//! program, and its memory is the process's to reserve.
//!
//! Why a thread of the guest's own: a guest's `Global`s are sound on one
//! thread (xpute_guest::global), and the threads that call into the guest
//! are the program's, as many as it has, each with the stack it was given.
//! The guest's thread is one, its stack sized for the guest.
//!
//! One guest a process, since the base every offset is from is one
//! (xpute_guest::mem::base): `boot` runs once with the guest's numbers, and
//! only what it returns reaches the guest.
//!
//! What a program writes is its allocator and one `native_abi!` with its
//! prefix, its numbers and its guest's entry, as the crate's example shows:
//! the macro boots the guest on the first call and exports the two calls
//! every such guest has, a turn and its memory, under the prefix, since a
//! process has one symbol namespace. What else the program's app calls is
//! the program's to export, over the `guest()` the macro defines.

mod alloc;
mod reserve;
mod thread;

use core::ptr::NonNull;
use core::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::time::Instant;

use xpute_guest::mem::heap::HeapStat;
use xpute_guest::mem::section::PAGE_BYTES;

pub use alloc::Alloc;

/// The guest's numbers, which the host boots it with.
pub struct Plan {
    /// The core memory's size, reserved once and never grown.
    pub memory_bytes: u32,
    /// Where the guest's heap begins, as an offset; it runs to the memory's
    /// end. What lies below it is the guest's to place. A whole number of
    /// the heap's pages (`xpute_guest::mem::section::PAGE_BYTES`): the slab
    /// aligns a block by its offset from here.
    pub heap_at: u32,
    /// The guest's thread's stack.
    pub stack_bytes: usize,
    /// The guest's thread's name, as a debugger and a crash report show it.
    pub thread_name: &'static str,
    /// The clock the guest reads (`xpute_guest::clock`), installed on its
    /// thread before its first call.
    pub clock: fn() -> f64,
}

/// The booted guest: the only way to call into it. `boot` makes the one.
#[derive(Clone, Copy)]
pub struct Guest {
    _booted: (),
}

static BOOTED: AtomicBool = AtomicBool::new(false);

/// Boots the guest as `plan` says: reserves its memory, starts its thread,
/// and there marks the thread as the guest's, sets the memory's base and
/// installs the clock, all before it returns. Once a process; a second boot
/// panics, and so does a plan whose heap does not begin on a page inside the
/// memory, before anything is reserved.
pub fn boot(plan: &Plan) -> Guest {
    assert!(plan.heap_at < plan.memory_bytes, "the heap begins at {} of a {}-byte memory", plan.heap_at, plan.memory_bytes);
    assert!(plan.heap_at.is_multiple_of(PAGE_BYTES), "the heap begins at {}, not on a {PAGE_BYTES}-byte page", plan.heap_at);
    assert!(!BOOTED.swap(true, Ordering::AcqRel), "a guest is booted once a process");
    let (at, bytes) = reserve::reserve(plan.memory_bytes as usize);
    alloc::bound(plan.heap_at as usize);
    let clock = plan.clock;
    thread::start(plan.thread_name, plan.stack_bytes, move || {
        xpute_guest::global::mark_thread();
        xpute_guest::mem::base::set_base(at, bytes);
        xpute_guest::clock::set_clock(clock);
    });
    Guest { _booted: () }
}

impl Guest {
    /// Runs `f` on the guest's thread and returns what it returns, the
    /// caller waiting: one of the two runs at a time. From the guest's own
    /// thread, or inside a `while_idle` on this one, it would wait for
    /// itself, and panics.
    pub fn run<R: Send>(self, f: impl FnOnce() -> R + Send) -> R {
        thread::run(f)
    }

    /// Runs `f` on this thread while the guest's runs nothing, for what must
    /// happen on a given thread but touches the guest. From the guest's own
    /// thread, or inside another `run` or `while_idle` on this one, it would
    /// wait for itself, and panics.
    pub fn while_idle<R>(self, f: impl FnOnce() -> R) -> R {
        thread::while_idle(f)
    }

    /// The core memory: its first byte and its bytes. What is read or written
    /// through it is read or written between the guest's turns, and never as
    /// one reference over the whole: the host's own blocks may lie in it.
    pub fn memory(self) -> (NonNull<u8>, usize) {
        reserve::reserved()
    }

    /// The guest's heap's reading.
    pub fn heap_stat(self) -> HeapStat {
        alloc::stat()
    }
}

/// The process's clock, in milliseconds from its first reading: what a
/// turn's quota is spent against, of which only differences are read.
pub fn now() -> f64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64() * 1000.0
}

/// A guest's boot and the C ABI every native guest has, under the program's
/// prefix: what a program would otherwise copy from guest to guest.
///
/// `native_abi!("demo", plan, entry)` defines, where it is written:
///
/// - `guest()`, the `Guest` booted with `plan` (a `Plan` expression,
///   evaluated once) on the first call, from whichever thread makes it;
/// - `interrupt`, exported as `demo_interrupt(double quota_ms) -> double`:
///   `entry(quota_ms)` run on the guest's thread, a turn of `quota_ms` and
///   when the guest wants the next;
/// - `memory`, exported as `demo_memory(uint32_t *bytes) -> uint8_t *`: the
///   memory's first byte, with its bytes written through `bytes`, which may
///   be null.
///
/// The prefix is the program's, since a process has one symbol namespace;
/// the Rust names are the macro's, for the program's own calls over them.
#[macro_export]
macro_rules! native_abi {
    ($prefix:literal, $plan:expr, $entry:expr $(,)?) => {
        /// The guest, booted with the program's plan on the first call, from
        /// whichever thread makes it.
        pub fn guest() -> $crate::native::Guest {
            static GUEST: ::std::sync::OnceLock<$crate::native::Guest> = ::std::sync::OnceLock::new();
            *GUEST.get_or_init(|| $crate::native::boot(&$plan))
        }

        /// A turn of `quota_ms` on the guest's thread, and when the guest
        /// wants the next.
        #[unsafe(export_name = concat!($prefix, "_interrupt"))]
        pub extern "C" fn interrupt(quota_ms: f64) -> f64 {
            guest().run(move || ($entry)(quota_ms))
        }

        /// The memory's first byte, with its bytes written through `bytes`.
        ///
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
