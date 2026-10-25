// xpute-host/examples/native.rs

//! A guest linked into its host's own native program, and the C ABI the
//! program's app calls it through. `native` is unix's, so elsewhere the
//! example is an empty `main`.

#[cfg(unix)]
mod program {
    use xpute_host::native::{self, Alloc, BootInfo, Plan};

    /// The program's one allocator: what the guest's thread asks for lies in
    /// the guest's memory, what any other thread asks for is the system's.
    #[global_allocator]
    static ALLOC: Alloc = Alloc;

    const MEMORY_BYTES: u32 = 16 << 20;

    // The guest booted with the program's numbers on the first call, and its
    // C ABI under xpute's names: `xpute_intr` and `xpute_mem`.
    xpute_host::native_abi!(
        Plan {
            memory_bytes: MEMORY_BYTES,
            data_end: 0,
            heap_at: 16 * xpute_guest::mem::section::PAGE_BYTES,
            stack_bytes: 1 << 20,
            thread_name: "demo",
            clock: native::now,
        },
        turn
    );

    /// The guest's own entry, as its crate exports it: a turn of `quota_ms`,
    /// with what the host hands it, and when it wants the next (negative for
    /// none).
    fn turn(quota_ms: f64, boot: fn() -> BootInfo) -> f64 {
        assert_eq!(boot().given, MEMORY_BYTES);
        let held = vec![quota_ms; 8];
        core::hint::black_box(held);
        -1.0
    }

    pub fn main() {
        let mut bytes = 0;
        // SAFETY: a word of our own.
        let at = unsafe { xpute_mem(&raw mut bytes) };
        assert!(!at.is_null());
        assert_eq!(bytes, MEMORY_BYTES);
        assert_eq!(xpute_intr(4.0), -1.0);
        assert!(guest().heap_stat().requests > 0, "the turn's block is the guest's heap's");
    }
}

fn main() {
    #[cfg(unix)]
    program::main();
}
