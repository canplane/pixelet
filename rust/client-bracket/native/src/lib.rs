// pxl-native/lib.rs

//! The kernel on a device: pxl-kernel linked into the app, a guest of
//! xpute's native host (xpute-host native), which reserves its memory, runs
//! every call into it on its own thread and keeps its blocks in its memory.
//! What is pixelet's here: its numbers (mem.json's and the thread's), the
//! kernel's door (pxl_kernel::door) as the guest's entry, the device the
//! renderer's calls are run on and what the views hand it to draw into, on
//! xpute's GPU host (surface.rs), the last words, and the transport's
//! decoder (unpack.rs).
//!
//! Called from the app through the C ABI (include/pxl_native.h), one call at
//! a time, from more than one thread: the JavaScript's rings the doorbell,
//! the main thread binds a view's layer.

#[cfg(target_os = "android")]
mod android;
mod surface;
mod unpack;

use pxl_kernel::door::layout::heap_at;
use pxl_kernel::sys::mem::spec::{GLOBAL_BASE, PHONE_BYTES, THREAD_STACK_BYTES};
use pxl_kernel::sys::stop::Stop;
use xpute_host::mem::Memory;
use xpute_host::native::{self, Alloc, BootInfo, Plan};

/// The library's one allocator: the kernel's blocks in its memory, so a
/// `Vec` the kernel hands the shell lies in the memory the shell reads.
#[global_allocator]
static ALLOC: Alloc = Alloc;

// The kernel, booted on the first call — a phone's reservation (mem.json's),
// as a page on a phone reserves, its own ranges past the rings' front
// (GLOBAL_BASE, door/layout.rs: a device's library keeps its statics where
// the linker put them) and its heap past `.fixed`, as the module's is — and
// the doorbell and the memory the shell reads the directory and the rings
// through, exported under xpute's names as `xpute_intr` and `xpute_mem`
// (include/pxl_native.h), and `guest()` here.
xpute_host::native_abi!(
    Plan {
        memory_bytes: PHONE_BYTES,
        data_end: GLOBAL_BASE,
        heap_at: heap_at(GLOBAL_BASE),
        stack_bytes: THREAD_STACK_BYTES as usize,
        thread_name: "pxl-kernel",
        clock: native::now,
    },
    turn
);

/// The process's error stream, or Android's log where that stream goes
/// nowhere: the kernel stops with the app, before the shell could run the
/// report's record.
fn last_words(stop: Stop<'_>) {
    let line = match stop {
        Stop::Bug { errnum, at, a, b } => format!(
            "[pxl] {errnum:?} at {}:{}:{} ({a}, {b})",
            at.file(),
            at.line(),
            at.column()
        ),
        Stop::Panic(info) => format!("[pxl] {info}"),
    };
    #[cfg(target_os = "android")]
    android::say(&line);
    #[cfg(not(target_os = "android"))]
    eprintln!("{line}");
}

/// The memory as the GPU's host reads and writes it between turns.
fn view() -> Memory<'static> {
    let (base, bytes) = guest().memory();
    // SAFETY: the memory, reserved for the process's life; between turns, on
    // the kernel's thread, where nothing holds a reference into a range the
    // kernel names.
    unsafe { Memory::from_raw(base, bytes) }
}

/// The doorbell's turn (pxl_kernel::door::intr), on the kernel's thread: a
/// turn of `quota_ms`, and when the kernel wants the next. The first lays
/// the memory out, on what the host hands it and with the last words.
/// Around it, the GPU's host: what the device answered written in before,
/// the turn's calls run on it after (surface.rs), and the frames it admits.
fn turn(quota_ms: f64, boot_info: fn() -> BootInfo) -> f64 {
    surface::before_turn(&mut view());
    let wake = pxl_kernel::door::intr(quota_ms, boot_info, Some(last_words), surface::admit);
    surface::after_turn(&mut view());
    wake
}
