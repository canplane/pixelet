// xpute-host/native/reserve.rs

//! The core memory: mapped once from the system, zero until touched, never
//! given back.

use std::sync::OnceLock;

use xpute_guest::mem::section::PAGE_BYTES;

/// Where the memory lies, its bytes, and the heap's offset in it.
#[derive(Clone, Copy)]
pub struct Core {
    pub at: usize,
    pub bytes: usize,
    pub heap_at: usize,
}

impl Core {
    pub fn holds(self, p: *const u8) -> bool {
        (self.at..self.at + self.bytes).contains(&p.addr())
    }
}

static CORE: OnceLock<Core> = OnceLock::new();

/// Once a process. A mapping lies on the system's page, which is all the
/// heap aligns to.
pub fn map(bytes: usize, heap_at: usize) -> Core {
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
        "the system would not map a guest's {bytes} bytes"
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

/// The memory, none before it is reserved.
pub fn core() -> Option<Core> {
    CORE.get().copied()
}
