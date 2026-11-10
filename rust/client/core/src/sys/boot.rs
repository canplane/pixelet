// pxl-core/sys/boot.rs

//! What the runtime hands the core on its first turn (door.rs `boot`), kept
//! for the core's life. A reader before the first turn gets a default.

use xpute_core::entry::BootInfo;
use xpute_core::global::Global;
use xpute_core::mem::heap::HeapStat;

static BOOT: Global<Option<BootInfo>> = Global::new();

pub fn set(boot: BootInfo) {
    BOOT.set(Some(boot));
}

/// The heap's counters, read through the function the runtime handed at
/// boot. Before the first turn every counter is zero.
pub fn heap_stat() -> HeapStat {
    BOOT.get(|| None)
        .map_or_else(HeapStat::default, |b| (b.heap_stat)())
}

#[cfg(test)]
#[path = "boot.test.rs"]
pub mod test;
