// xpute-guest/wasm.rs

//! The guest built as one WebAssembly module: how its build script links it
//! (`link`), what it reads of the module (`data_end`, `stack_high`, `given`,
//! `now`), and `wasm_abi!`, which defines its allocator and exports `intr`
//! (xpute-host `native_abi!` is the native form's).
//!
//! Only the module's reads are wasm32's: a build script runs on the build
//! machine. Nothing here is a global allocator or an export until a program
//! writes the macro, since a build dependency on this crate is compiled apart
//! from the guest's own (resolver 2) and must not be one.
//!
//! ```ignore
//! xpute_guest::wasm_abi!(
//!     xpute_guest::wasm::Plan {
//!         heap_at: |data_end| data_end.next_multiple_of(PAGE_BYTES),
//!         stack_bytes: 1 << 20,
//!     },
//!     my_guest::intr
//! );
//! ```

mod link;
#[cfg(target_arch = "wasm32")]
mod module;

pub use link::{link, LinkPlan, WASM_PAGE_BYTES};
#[cfg(target_arch = "wasm32")]
pub use module::{data_end, given, now, stack_high};

/// What xpute must not assume of a module guest.
pub struct Plan {
    /// Where the heap begins, from where the linker stopped; it runs to the
    /// end of the memory given. Pure: the allocator asks on its first block,
    /// before anything has laid the memory out.
    pub heap_at: fn(data_end: u32) -> u32,
    /// The stack it was linked with (`LinkPlan::stack_bytes`), checked
    /// against `__stack_high` as it boots.
    pub stack_bytes: u32,
}

#[cfg(target_arch = "wasm32")]
pub fn boot_info(plan: &Plan, heap_stat: fn() -> crate::mem::heap::HeapStat) -> crate::entry::BootInfo {
    let high = stack_high();
    xpute_kit::ensure!(high == plan.stack_bytes, ENOTRECOVERABLE, high);
    crate::entry::BootInfo {
        given: u32::try_from(given()).unwrap_or(u32::MAX),
        data_end: data_end(),
        clock: now,
        heap_stat,
    }
}

/// The module's `#[global_allocator]` over `[plan.heap_at(data_end), given)`
/// and its one export, `intr`, which runs the program's `entry::Entry`.
#[macro_export]
macro_rules! wasm_abi {
    ($plan:expr, $entry:expr $(,)?) => {
        mod xpute_wasm {
            #[allow(unused_imports)]
            use super::*;

            const PLAN: $crate::wasm::Plan = $plan;
            const ENTRY: $crate::entry::Entry = $entry;

            struct WasmHeap;

            impl $crate::mem::heap::Bounds for WasmHeap {
                fn base() -> usize {
                    (PLAN.heap_at)($crate::wasm::data_end()) as usize
                }
                fn end() -> usize {
                    $crate::wasm::given() as usize
                }
            }

            #[global_allocator]
            static WASM_HEAP: $crate::mem::heap::Heap<WasmHeap> = $crate::mem::heap::Heap::new();

            fn boot_info() -> $crate::entry::BootInfo {
                $crate::wasm::boot_info(&PLAN, || $crate::mem::heap::heap_stat(&WASM_HEAP))
            }

            pub fn intr(quota_ms: f64) -> f64 {
                $crate::entry::intr(quota_ms, boot_info, ENTRY)
            }
        }

        #[no_mangle]
        pub extern "C" fn intr(quota_ms: f64) -> f64 {
            xpute_wasm::intr(quota_ms)
        }
    };
}
