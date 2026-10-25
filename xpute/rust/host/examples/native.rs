// xpute-host/examples/native.rs

//! A guest linked into its host's own native program, and the C ABI the
//! program's app calls it through.

use std::sync::OnceLock;

use xpute_host::native::{self, Alloc, Guest, Plan};

/// The program's one allocator: what the guest's thread asks for lies in the
/// guest's memory, what any other thread asks for is the system's.
#[global_allocator]
static ALLOC: Alloc = Alloc;

/// The guest, booted with the program's numbers on the first call, from
/// whichever thread makes it: a 16 MiB memory, its heap from 64 KiB on.
fn guest() -> Guest {
    static GUEST: OnceLock<Guest> = OnceLock::new();
    *GUEST.get_or_init(|| native::boot(&Plan::new(16 << 20, 1 << 16)))
}

/// The guest's own entry, as its crate exports it: a turn of `quota_ms`, and
/// when it wants the next (negative for none).
fn turn(quota_ms: f64) -> f64 {
    let held = vec![quota_ms; 8];
    core::hint::black_box(held);
    -1.0
}

/// A turn. The names carry the program's prefix: a process has one symbol
/// namespace.
#[no_mangle]
pub extern "C" fn demo_interrupt(quota_ms: f64) -> f64 {
    guest().run(move || turn(quota_ms))
}

/// The memory's first byte, with its bytes written through `bytes`.
///
/// # Safety
/// `bytes` is null or points at a `u32` the caller owns.
#[no_mangle]
pub unsafe extern "C" fn demo_memory(bytes: *mut u32) -> *mut u8 {
    let (at, n) = guest().memory();
    // SAFETY: the caller's word, or none.
    if let Some(bytes) = unsafe { bytes.as_mut() } {
        *bytes = n as u32;
    }
    at.as_ptr()
}

fn main() {
    let mut bytes = 0;
    // SAFETY: a word of our own.
    let at = unsafe { demo_memory(&raw mut bytes) };
    assert!(!at.is_null());
    assert_eq!(bytes, 16 << 20);
    assert_eq!(demo_interrupt(4.0), -1.0);
    assert!(guest().heap_stat().requests > 0, "the turn's block is the guest's heap's");
}
