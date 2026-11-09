// xpute-core/wasm.rs

//! This is the core built as one WebAssembly module: how its build script links
//! it (`link`), what it reads of the module itself (`data_end`, `stack_high`,
//! `given`, `now`), what it asks of its runtime (`call`), and `wasm_abi!`,
//! which defines the module's allocator and exports `run`. The native form's
//! counterpart is xpute-runtime's `native_abi!`.
//!
//! Only the module's reads are compiled for wasm32 alone, since a build
//! script runs on the build machine. Nothing here becomes a global allocator
//! or an export until a program invokes the macro: a build dependency on this
//! crate is compiled separately from the core's own (resolver 2), and must
//! define neither.
//!
//! ```ignore
//! xpute_core::wasm_abi!(
//!     xpute_core::wasm::Config {
//!         heap_at: |data_end| data_end.next_multiple_of(PAGE_BYTES),
//!         stack_bytes: 1 << 20,
//!         admit_frame: None,
//!     },
//!     my_core::run
//! );
//! ```

mod link;
#[cfg(target_arch = "wasm32")]
mod module;

pub use link::{link, LinkConfig, WASM_PAGE_BYTES};
#[cfg(target_arch = "wasm32")]
pub use module::{call, data_end, mem_sz, now, stack_high};

/// xpute cannot assume these of a module core, and the program supplies them.
pub struct Config {
    /// Where the heap begins, computed from where the linker's data ends; the
    /// heap runs to the end of the memory given. It must be pure: the
    /// allocator calls it for its first block, before anything has laid out
    /// the memory.
    pub heap_at: fn(data_end: u32) -> u32,
    /// The stack size the module was linked with (`LinkConfig::stack_bytes`).
    /// Boot checks it against `__stack_high`.
    pub stack_bytes: u32,
    /// What the core is handed as `Form::admit_frame`: the page's admission
    /// where the core draws (xpute-gpu `admit::page`, which `core!` passes
    /// when given the GPU), and none where it draws nothing.
    pub admit_frame: Option<fn() -> bool>,
}

#[cfg(target_arch = "wasm32")]
pub fn boot_info(
    cfg: &Config,
    heap_stat: fn() -> crate::mem::heap::HeapStat,
) -> crate::entry::BootInfo {
    let high = stack_high();
    xpute_kit::ensure!(high == cfg.stack_bytes, ENOTRECOVERABLE, high);
    crate::entry::BootInfo {
        mem_sz: u32::try_from(mem_sz()).unwrap_or(u32::MAX),
        call,
        heap_stat,
        form: crate::entry::Form {
            data_end: data_end(),
            now,
            stop_hook: None,
            admit_frame: cfg.admit_frame,
            publish: None,
        },
    }
}

/// Defines the module's `#[global_allocator]` over
/// `[cfg.heap_at(data_end), given)` and its one export, `run`, which runs the
/// program's `entry::Entry`.
#[macro_export]
macro_rules! wasm_abi {
    ($cfg:expr, $entry:expr $(,)?) => {
        mod xpute_wasm {
            #[allow(unused_imports)]
            use super::*;

            const CFG: $crate::wasm::Config = $cfg;
            const ENTRY: $crate::entry::Entry = $entry;

            struct WasmHeap;

            impl $crate::mem::heap::Bound for WasmHeap {
                fn base() -> usize {
                    (CFG.heap_at)($crate::wasm::data_end()) as usize
                }
                fn end() -> usize {
                    $crate::wasm::mem_sz() as usize
                }
            }

            #[global_allocator]
            static WASM_HEAP: $crate::mem::heap::Heap<WasmHeap> = $crate::mem::heap::Heap::new();

            fn boot_info() -> $crate::entry::BootInfo {
                $crate::wasm::boot_info(&CFG, || $crate::mem::heap::heap_stat(&WASM_HEAP))
            }

            pub fn run(deadline_ms: f64) -> f64 {
                $crate::entry::run(deadline_ms, boot_info, ENTRY)
            }
        }

        #[no_mangle]
        pub extern "C" fn run(deadline_ms: f64) -> f64 {
            xpute_wasm::run(deadline_ms)
        }
    };
}
