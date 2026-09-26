// pxl-native/lib.rs

//! The kernel on a device: pxl-kernel linked into the app, where pxl-wasm
//! instantiates it in a page. The same door (pxl_kernel::door) behind a
//! different bracket — the memory is a region this library takes from the
//! system (memory.rs), the clock is the device's, and the renderer draws
//! rather than recording its calls for a host to run.
//!
//! Called from the app through the C ABI (include/pxl_native.h), one call at
//! a time, and run on the kernel's own thread (thread.rs) — the single
//! thread `Global` is sound for. What reaches nothing a `Global` holds stays
//! on the caller's: an image decoded (image.rs), behind a lock of its own.

mod image;
mod memory;
mod surface;
mod thread;

use thread::on_kernel;

use std::sync::OnceLock;
use std::time::Instant;

use pxl_kernel::door::Bracket;

/// The device's clock, in milliseconds from the first reading: a turn's quota
/// is spent against it, and only differences are read.
fn now() -> f64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64() * 1000.0
}

fn heap_stat() -> pxl_kernel::host::HeapStat {
    memory::ALLOC.stat()
}

/// The process's error stream: the kernel stops with the app, before the
/// shell could run the report's record.
fn last_words(what: &dyn core::fmt::Display) {
    eprintln!("[pxl] {what}");
}

/// What a device gives the kernel: the region, the clock and the allocator.
fn bracket() -> Bracket {
    memory::region();
    Bracket {
        given: memory::GIVEN,
        data_end: memory::DATA_END,
        clock: now,
        heap_stat,
        last_words: Some(last_words),
    }
}

/// The doorbell (pxl_kernel::door::interrupt): a turn of `quota_ms`, and when
/// the kernel wants the next. The first lays the memory out.
#[no_mangle]
pub extern "C" fn pxl_interrupt(quota_ms: f64) -> f64 {
    on_kernel(move || pxl_kernel::door::interrupt(quota_ms, bracket))
}

/// The memory: its first byte, with its bytes written through `bytes` — what
/// the shell reads the boot block and the rings through.
///
/// # Safety
/// `bytes` is null or points at a `u32` the caller owns.
#[no_mangle]
pub unsafe extern "C" fn pxl_memory(bytes: *mut u32) -> *mut u8 {
    let at = on_kernel(|| {
        memory::region();
        xpute_runtime::mem::base::ptr_at::<u8>(0).expose_provenance()
    });
    if !bytes.is_null() {
        // SAFETY: the caller's word.
        unsafe { *bytes = memory::GIVEN };
    }
    core::ptr::with_exposed_provenance_mut(at)
}
