// pxl-kernel/sys/boot.rs

//! What the kernel was booted with, kept from its first turn (door.rs
//! `boot`): what the bracket's form handed it, and its GPU host's admission.

use xpute_gpu::admit::Admit;
use xpute_guest::entry::BootInfo;
use xpute_guest::global::Global;
use xpute_guest::mem::heap::HeapStat;

static BOOT: Global<Option<BootInfo>> = Global::new();

pub fn set(boot: BootInfo) {
    BOOT.set(Some(boot));
}

/// A test's heap reading, as a bracket would hand it.
#[cfg(test)]
pub fn set_heap_stat(heap_stat: fn() -> HeapStat) {
    set(BootInfo {
        given: 0,
        data_end: 0,
        clock: || 0.0,
        heap_stat,
    });
}

pub fn heap_stat() -> HeapStat {
    BOOT.get(|| None)
        .map_or_else(HeapStat::default, |b| (b.heap_stat)())
}

/// Whether the GPU's host takes a frame now (xpute_gpu::admit), as the
/// bracket hands it with the entry; every frame until it has.
static ADMIT: Global<Admit> = Global::new();

pub fn set_admit(admit: Admit) {
    ADMIT.set(admit);
}

pub fn admit() -> Admit {
    *ADMIT.get(|| xpute_gpu::admit::every)
}
