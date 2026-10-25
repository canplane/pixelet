// pxl-native/lib.rs

//! The kernel on a device: pxl-kernel linked into the app, a guest of
//! xpute's native host (xpute-host native), which reserves its memory, runs
//! every call into it on its own thread and keeps its blocks in its memory.
//! What is pixelet's here: its numbers (mem.json's and the thread's), the
//! kernel's door (pxl_kernel::door) as the guest's entry, the device and the
//! views' surfaces the renderer's calls are run on (surface.rs), the last
//! words, and the transport's decoder (unpack.rs).
//!
//! Called from the app through the C ABI (include/pxl_native.h), one call at
//! a time, from more than one thread: the JavaScript's rings the doorbell,
//! the main thread binds a view's layer.

#[cfg(target_os = "android")]
mod android;
mod surface;
mod unpack;

use pxl_kernel::door::layout::heap_at;
use pxl_kernel::door::Bracket;
use pxl_kernel::sys::io::host::Stop;
use pxl_kernel::sys::mem::spec::{GLOBAL_BASE, PHONE_BYTES};
use xpute_guest::mem::heap::HeapStat;
use xpute_host::mem::Memory;
use xpute_host::native::{self, Alloc, Plan};

/// The library's one allocator: the kernel's blocks in its memory, so a
/// `Vec` the kernel hands the shell lies in the memory the shell reads.
#[global_allocator]
static ALLOC: Alloc = Alloc;

/// The kernel's thread's stack. React Native's JavaScript thread has 1 MiB,
/// and a turn that draws through wgpu can take more
/// (doc/minutes/archive/20260926-a-device-s-kernel-on-a-thread-of-its-own.md);
/// how deep wgpu's internals go has no bound this crate can state, so the
/// kernel's thread has a main thread's 8 MiB.
const KERNEL_STACK_BYTES: usize = 8 << 20;

// The kernel, booted on the first call — a phone's reservation (mem.json's),
// as a page on a phone reserves, its heap past the rings' front (GLOBAL_BASE,
// door/layout.rs) and `.fixed`, as the module's is — and the doorbell and the
// memory the shell reads the directory and the rings through, exported as
// `pxl_interrupt` and `pxl_memory` (include/pxl_native.h): `guest()`,
// `interrupt` and `memory` here.
xpute_host::native_abi!(
    "pxl",
    Plan {
        thread_name: "pxl-kernel",
        stack_bytes: KERNEL_STACK_BYTES,
        ..Plan::new(PHONE_BYTES, heap_at(GLOBAL_BASE))
    },
    turn
);

/// Runs `f` on the kernel's thread, the caller waiting.
fn on_kernel<R: Send>(f: impl FnOnce() -> R + Send) -> R {
    guest().run(f)
}

fn heap_stat() -> HeapStat {
    guest().heap_stat()
}

/// The process's error stream, or Android's log where that stream goes
/// nowhere: the kernel stops with the app, before the shell could run the
/// report's record.
fn last_words(stop: Stop<'_>) {
    let line = match stop {
        Stop::Bug { errno, at, a, b } => format!("[pxl] {errno:?} at {}:{}:{} ({a}, {b})", at.file(), at.line(), at.column()),
        Stop::Panic(info) => format!("[pxl] {info}"),
    };
    #[cfg(target_os = "android")]
    android::say(&line);
    #[cfg(not(target_os = "android"))]
    eprintln!("{line}");
}

/// What a device gives the kernel: the memory, its data's end at the rings'
/// front (a device's library keeps its statics where the linker put them),
/// the clock, the allocator and the frames its surfaces admit (surface.rs).
fn bracket() -> Bracket {
    Bracket {
        given: guest().memory().1 as u32,
        data_end: GLOBAL_BASE,
        clock: native::now,
        heap_stat,
        admit_frame: surface::admit_frame,
        last_words: Some(last_words),
    }
}

/// The memory as the GPU's host reads and writes it between turns.
fn view() -> Memory<'static> {
    let (base, bytes) = guest().memory();
    // SAFETY: the memory, reserved for the process's life; between turns, on
    // the kernel's thread, where nothing holds a reference into a range the
    // kernel names.
    unsafe { Memory::from_raw(base, bytes) }
}

/// The doorbell's turn (pxl_kernel::door::interrupt), on the kernel's thread:
/// a turn of `quota_ms`, and when the kernel wants the next. The first lays
/// the memory out. Around it, the GPU's host: what the device answered
/// written in before, the turn's calls run on it after (surface.rs).
fn turn(quota_ms: f64) -> f64 {
    surface::before_turn(&mut view());
    let wake = pxl_kernel::door::interrupt(quota_ms, bracket);
    surface::after_turn(&mut view());
    wake
}
