// pxl-kernel/sys/boot.rs

//! What the kernel was booted with, kept from its first turn (door.rs
//! `boot`): what its runtime's form handed it, its GPU runtime's admission among it.

use xpute_core::entry::BootInfo;
use xpute_core::global::Global;
use xpute_core::mem::heap::HeapStat;
use xpute_gpu::admit;
use xpute_gpu::admit::Admit;
#[cfg(test)]
use xpute_kit::status::errno::Errno;

static BOOT: Global<Option<BootInfo>> = Global::new();

pub fn set(boot: BootInfo) {
    BOOT.set(Some(boot));
}

/// A test's heap reading, as a runtime's form would hand it.
#[cfg(test)]
pub fn set_heap_stat(heap_stat: fn() -> HeapStat) {
    set(BootInfo {
        given: 0,
        data_end: 0,
        clock: || 0.0,
        call: |_| Errno::ENOSYS as i32,
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

/// Whether the GPU's runtime takes a frame now (xpute_gpu::admit), as its
/// runtime's form hands it (`BootInfo::admit`); every frame where it hands none
/// or before the first turn.
pub fn admit() -> Admit {
    BOOT.get(|| None)
        .and_then(|b| b.admit)
        .unwrap_or(admit::every)
}
