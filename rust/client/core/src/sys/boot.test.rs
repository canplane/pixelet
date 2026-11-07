use core::sync::atomic::{AtomicU32, Ordering};

use super::{set_heap_stat, HeapStat};

/// How many times the heap `set_counted_heap` sets has given memory back.
static RELEASES: AtomicU32 = AtomicU32::new(0);

/// A test's heap that gives memory back only when the test says so
/// (`heap_release`).
pub fn set_counted_heap() {
    set_heap_stat(|| HeapStat {
        releases: RELEASES.load(Ordering::Relaxed),
        ..HeapStat::default()
    });
}

/// The heap `set_counted_heap` sets gives memory back once.
pub fn heap_release() {
    RELEASES.fetch_add(1, Ordering::Relaxed);
}
