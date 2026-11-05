// pxl-kernel/sys/boot.rs

//! What the kernel was booted with, kept from its first turn (door.rs
//! `boot`): what its host's form handed it, its GPU host's admission among it.

use xpute_gpu::admit;
use xpute_gpu::admit::Admit;
use xpute_guest::entry::BootInfo;
use xpute_guest::global::Global;
use xpute_guest::mem::heap::HeapStat;
#[cfg(test)]
use xpute_kit::status::errno::Errno;

static BOOT: Global<Option<BootInfo>> = Global::new();

pub fn set(boot: BootInfo) {
    BOOT.set(Some(boot));
}

/// A test's heap reading, as a host's form would hand it.
#[cfg(test)]
pub fn set_heap_stat(heap_stat: fn() -> HeapStat) {
    set(BootInfo {
        given: 0,
        data_end: 0,
        clock: || 0.0,
        hostcall: |_| Errno::ENOSYS as i32,
        heap_stat,
        stop_hook: None,
        admit: None,
        publish: None,
    });
}

pub fn heap_stat() -> HeapStat {
    BOOT.get(|| None)
        .map_or_else(HeapStat::default, |b| (b.heap_stat)())
}

/// Whether the GPU's host takes a frame now (xpute_gpu::admit), as its
/// host's form hands it (`BootInfo::admit`); every frame where it hands none
/// or before the first turn.
pub fn admit() -> Admit {
    BOOT.get(|| None)
        .and_then(|b| b.admit)
        .unwrap_or(admit::every)
}
