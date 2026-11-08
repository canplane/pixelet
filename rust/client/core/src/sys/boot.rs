// pxl-core/sys/boot.rs

//! What the core was booted with, kept from its first turn (door.rs
//! `boot`): what its runtime's form handed it, its GPU runtime's admission
//! among it.

use xpute_core::entry::BootInfo;
use xpute_core::global::Global;
use xpute_core::mem::heap::HeapStat;
use xpute_gpu::admit;
use xpute_gpu::admit::Admit;

static BOOT: Global<Option<BootInfo>> = Global::new();

pub fn set(boot: BootInfo) {
    BOOT.set(Some(boot));
}

/// The heap's reading, as the runtime's form hands it; none counted before
/// the first turn.
pub fn heap_stat() -> HeapStat {
    BOOT.get(|| None)
        .map_or_else(HeapStat::default, |b| (b.heap_stat)())
}

/// Whether the GPU's runtime takes a frame now (xpute_gpu::admit), as its
/// runtime's form hands it (`Form::admit_frame`); every frame where it hands
/// none, or before the first turn.
pub fn admit_frame() -> Admit {
    BOOT.get(|| None)
        .and_then(|b| b.form.admit_frame)
        .unwrap_or(admit::every)
}

#[cfg(test)]
#[path = "boot.test.rs"]
pub mod test;
