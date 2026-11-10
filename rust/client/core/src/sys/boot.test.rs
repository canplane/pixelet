// pxl-core/sys/boot.test.rs
//
// Boots the core for a test with heap counters the test supplies, in place
// of the ones a runtime would hand it.

use core::sync::atomic::{AtomicU32, Ordering};

use xpute_core::entry::{BootInfo, Form};
use xpute_core::mem::heap::HeapStat;
use xpute_kit::status::errno::Errno;

use super::set;

/// How many times the heap of `set_counted_heap` has given memory back.
static RELEASES: AtomicU32 = AtomicU32::new(0);

pub fn set_heap_stat(heap_stat: fn() -> HeapStat) {
    set(BootInfo {
        mem_sz: 0,
        call: |_| Errno::ENOSYS as i32,
        heap_stat,
        form: Form {
            data_end: 0,
            now: || 0.0,
            stop_hook: None,
            publish: None,
        },
    });
}

/// Boots with a heap that gives memory back only when the test calls
/// `heap_release`.
pub fn set_counted_heap() {
    set_heap_stat(|| HeapStat {
        releases: RELEASES.load(Ordering::Relaxed),
        ..HeapStat::default()
    });
}

pub fn heap_release() {
    RELEASES.fetch_add(1, Ordering::Relaxed);
}
