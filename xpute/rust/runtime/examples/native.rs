// xpute-runtime/examples/native.rs

//! This is a core linked into its runtime's own native program, and the C ABI
//! through which the program's app calls it. `native` exists only on unix, so
//! elsewhere the example is an empty `main`.

#[cfg(unix)]
mod program {
    use xpute_runtime::native::{self, Alloc, BootInfo, Config};

    /// Blocks the core's thread allocates lie in the core's memory; any other
    /// thread's blocks come from the system.
    #[global_allocator]
    static ALLOC: Alloc = Alloc;

    const MEM_BYTES: u32 = 16 << 20;

    xpute_runtime::native_abi!(
        Config {
            mem_bytes: MEM_BYTES,
            heap_at: 16 * xpute_core::mem::section::PAGE_BYTES,
            stack_bytes: 1 << 20,
            thread_name: "demo",
            ready: || true,
            form: xpute_runtime::native::Form {
                data_end: 0,
                now: native::now,
                stop_hook: None,
                admit_frame: None,
                publish: None,
            },
        },
        turn
    );

    /// The core's entry (xpute_core::entry `Entry`). Its block comes from the
    /// core's heap, and it answers -1 to ask for no further turn.
    fn turn(deadline_ms: f64, boot: fn() -> BootInfo) -> f64 {
        assert_eq!(boot().mem_sz, MEM_BYTES);
        let buf = vec![deadline_ms; 8];
        core::hint::black_box(buf);
        -1.0
    }

    pub(crate) fn main() {
        let mut bytes = 0;
        // SAFETY: a word of our own.
        let at = unsafe { xpute_mem(&raw mut bytes) };
        assert!(!at.is_null());
        assert_eq!(bytes, MEM_BYTES);
        assert_eq!(xpute_run(4.0), -1.0);
        assert!(
            core().heap_stat().requests > 0,
            "the turn's block is the core's heap's"
        );
    }
}

fn main() {
    #[cfg(unix)]
    program::main();
}
