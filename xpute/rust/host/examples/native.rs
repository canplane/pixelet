// xpute-host/examples/native.rs

//! A guest linked into its host's own native program, and the C ABI the
//! program's app calls it through. `native` is unix's, so elsewhere the
//! example is an empty `main`.

#[cfg(unix)]
mod program {
    use xpute_host::native::{Alloc, Plan};

    /// The program's one allocator: what the guest's thread asks for lies in
    /// the guest's memory, what any other thread asks for is the system's.
    #[global_allocator]
    static ALLOC: Alloc = Alloc;

    // The guest booted with the program's numbers on the first call — a 16
    // MiB memory, its heap from 64 KiB on, a whole number of pages — and its
    // C ABI under the program's prefix: `demo_interrupt` and `demo_memory`.
    // The plan's clock, `native::now` unless it says otherwise, is installed
    // on the guest's thread before its first call.
    xpute_host::native_abi!("demo", Plan::new(16 << 20, 1 << 16), turn);

    /// The guest's own entry, as its crate exports it: a turn of `quota_ms`,
    /// and when it wants the next (negative for none).
    fn turn(quota_ms: f64) -> f64 {
        let held = vec![quota_ms; 8];
        core::hint::black_box(held);
        -1.0
    }

    pub fn main() {
        let mut bytes = 0;
        // SAFETY: a word of our own.
        let at = unsafe { memory(&raw mut bytes) };
        assert!(!at.is_null());
        assert_eq!(bytes, 16 << 20);
        assert_eq!(interrupt(4.0), -1.0);
        assert!(guest().heap_stat().requests > 0, "the turn's block is the guest's heap's");
    }
}

fn main() {
    #[cfg(unix)]
    program::main();
}
