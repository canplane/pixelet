// xpute-runtime/native/reserve.rs

//! The core's memory is mapped once from the system. It reads as zero until it
//! is written, and it is never given back.

use std::sync::OnceLock;

use xpute_core::mem::section::PAGE_BYTES;

#[derive(Clone, Copy)]
pub(super) struct Core {
    pub at: usize,
    pub bytes: usize,
    pub heap_at: usize,
}

impl Core {
    pub(super) fn holds(self, p: *const u8) -> bool {
        (self.at..self.at + self.bytes).contains(&p.addr())
    }
}

static CORE: OnceLock<Core> = OnceLock::new();

/// Maps the memory, once a process. The system places a mapping on a page
/// boundary, and the heap needs no stricter alignment than that.
pub(super) fn map(bytes: usize, heap_at: usize) -> Core {
    // SAFETY: an anonymous private mapping, which touches nothing that exists.
    let p = unsafe {
        libc::mmap(
            core::ptr::null_mut(),
            bytes,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_PRIVATE | libc::MAP_ANON,
            -1,
            0,
        )
    };
    assert!(
        p != libc::MAP_FAILED,
        "the system would not map a core's {bytes} bytes"
    );
    let at = p.cast::<u8>().expose_provenance();
    assert!(
        at.is_multiple_of(PAGE_BYTES as usize),
        "the system mapped the memory off its page"
    );
    let core = Core { at, bytes, heap_at };
    assert!(CORE.set(core).is_ok(), "the memory is reserved once");
    core
}

pub(super) fn core() -> Option<Core> {
    CORE.get().copied()
}
