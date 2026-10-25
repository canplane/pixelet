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
//! What a program writes is its allocator, its boot with its numbers, and
//! its C ABI under its own prefix, one line over `Guest` each, as the
//! crate's example shows. The C symbols are the program's to write out: they
//! carry its prefix, since a process has one symbol namespace, and
//! `macro_rules!` cannot join a prefix to a name, so a macro would be handed
//! every whole name and save no line.

mod alloc;
mod reserve;
mod thread;

use core::ptr::NonNull;
use core::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::time::Instant;

use xpute_guest::mem::heap::HeapStat;

pub use alloc::Alloc;

/// The guest's thread's stack unless the plan says otherwise: a main
/// thread's on Linux and macOS.
pub const STACK_BYTES: usize = 8 << 20;

/// The guest's numbers, which the host boots it with.
pub struct Plan {
    /// The core memory's size, reserved once and never grown.
    pub memory_bytes: u32,
    /// Where the guest's heap begins, as an offset; it runs to the memory's
    /// end. What lies below it is the guest's to place.
    pub heap_at: u32,
    /// The guest's thread's stack.
    pub stack_bytes: usize,
    /// The guest's thread's name, as a debugger and a crash report show it.
    pub thread_name: &'static str,
}

impl Plan {
    /// A memory of `memory_bytes`, its heap from `heap_at`, on a thread
    /// named `guest` with a stack of `STACK_BYTES`.
    pub const fn new(memory_bytes: u32, heap_at: u32) -> Plan {
        Plan {
            memory_bytes,
            heap_at,
            stack_bytes: STACK_BYTES,
            thread_name: "guest",
        }
    }
}

/// The booted guest: the only way to call into it. `boot` makes the one.
#[derive(Clone, Copy)]
pub struct Guest {
    _booted: (),
}

static BOOTED: AtomicBool = AtomicBool::new(false);

/// Boots the guest as `plan` says: reserves its memory, starts its thread,
/// and there marks the thread as the guest's and sets the memory's base, all
/// before it returns. Once a process; a second boot panics.
pub fn boot(plan: &Plan) -> Guest {
    assert!(!BOOTED.swap(true, Ordering::AcqRel), "a guest is booted once a process");
    assert!(plan.heap_at < plan.memory_bytes, "the heap begins at {} of a {}-byte memory", plan.heap_at, plan.memory_bytes);
    let (at, bytes) = reserve::reserve(plan.memory_bytes as usize);
    alloc::bound(plan.heap_at as usize);
    thread::start(plan.thread_name, plan.stack_bytes, move || {
        xpute_guest::global::mark_thread();
        xpute_guest::mem::base::set_base(at, bytes);
    });
    Guest { _booted: () }
}

impl Guest {
    /// Runs `f` on the guest's thread and returns what it returns, the
    /// caller waiting: one of the two runs at a time. From the guest's own
    /// thread it would wait for itself, and panics.
    pub fn run<R: Send>(self, f: impl FnOnce() -> R + Send) -> R {
        thread::run(f)
    }

    /// Runs `f` on this thread while the guest's runs nothing, for what must
    /// happen on a given thread but touches the guest.
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
