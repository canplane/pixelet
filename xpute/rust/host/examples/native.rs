// xpute-host/examples/native.rs

//! A guest linked into its host's own native program, and the C ABI the
//! program's app calls it through. `native` is unix's, so elsewhere the
//! example is an empty `main`.

#[cfg(unix)]
mod program {
    use xpute_host::native::{self, Alloc, BootInfo, Config};

    /// What the guest's thread asks for lies in its memory; any other
    /// thread's is the system's.
    #[global_allocator]
    static ALLOC: Alloc = Alloc;

    const MEM_BYTES: u32 = 16 << 20;

    xpute_host::native_abi!(
        Config {
            mem_bytes: MEM_BYTES,
            data_end: 0,
            heap_at: 16 * xpute_guest::mem::section::PAGE_BYTES,
            stack_bytes: 1 << 20,
            thread_name: "demo",
            clock: native::now,
        },
        turn
    );

    /// The guest's entry (xpute_guest::entry `Entry`).
    fn turn(quota_ms: f64, boot: fn() -> BootInfo) -> f64 {
        assert_eq!(boot().given, MEM_BYTES);
        let held = vec![quota_ms; 8];
        core::hint::black_box(held);
        -1.0
    }

    pub fn main() {
        let mut bytes = 0;
        // SAFETY: a word of our own.
        let at = unsafe { xpute_mem(&raw mut bytes) };
        assert!(!at.is_null());
        assert_eq!(bytes, MEM_BYTES);
        assert_eq!(xpute_intr(4.0), -1.0);
        assert!(
            guest().heap_stat().requests > 0,
            "the turn's block is the guest's heap's"
        );
    }
}

fn main() {
    #[cfg(unix)]
    program::main();
}
