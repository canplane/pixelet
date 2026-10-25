// xpute-guest/wasm.rs

//! The guest built as one WebAssembly module, the whole of that form: how
//! its build script links it (`link`, `LinkPlan`), what it reads of the
//! module itself (`data_end`, `stack_high`, `given`, `now`), and the
//! program's one `wasm_abi!`, which defines its allocator over the heap its
//! `Plan` names and exports its entry, `intr`. The counterpart of
//! xpute-host's `native` and its `native_abi!`.
//!
//! A build script runs on the build machine, so `link` carries no cfg; only
//! the module's reads are wasm32's. Nothing here is a global allocator or an
//! export until a program writes the macro: a build dependency on this crate
//! is compiled apart from the guest's own (resolver 2), and must not be one.
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

/// The guest's numbers as a module, which `wasm_abi!` runs it by: what xpute
/// must not assume of it.
pub struct Plan {
    /// Where the heap begins, from where the linker stopped (`data_end`): on
    /// a page, past whatever the guest keeps there. It runs to the end of the
    /// memory the host gave (`given`). A pure function: the allocator asks for
    /// it on its first block, before anything has laid the memory out.
    pub heap_at: fn(data_end: u32) -> u32,
    /// The stack it was linked with (`LinkPlan::stack_bytes`), which the
    /// linker's `__stack_high` is checked against as the guest boots.
    pub stack_bytes: u32,
}

/// What the module hands its guest (`entry::BootInfo`), its stack checked
/// first against the plan's: the memory the host grew it to, where the
/// linker stopped, the host's clock and `heap_stat`.
#[cfg(target_arch = "wasm32")]
pub fn boot_info(plan: &Plan, heap_stat: fn() -> crate::mem::heap::HeapStat) -> crate::entry::BootInfo {
    let high = stack_high();
    xpute_kit::ensure!(high == plan.stack_bytes, ENOTRECOVERABLE, high);
    crate::entry::BootInfo {
        given: given().min(u64::from(u32::MAX)) as u32,
        data_end: data_end(),
        clock: now,
        heap_stat,
    }
}

/// The module's allocator and its entry, written once by the program over
/// its `Plan` and its `entry::Entry`: the `#[global_allocator]` over
/// `[plan.heap_at(data_end), given)`, `boot_info()` — what the module hands
/// the guest, the program's for its own calls too — and the export `intr`,
/// a turn of `quota_ms` that answers when the guest wants the next.
#[macro_export]
macro_rules! wasm_abi {
    ($plan:expr, $entry:expr $(,)?) => {
        /// Where the module's heap lies: from where the plan begins it to the
        /// end of the memory the host gave.
        struct WasmHeap;

        impl $crate::mem::heap::Bounds for WasmHeap {
            fn base() -> usize {
                let plan: $crate::wasm::Plan = $plan;
                (plan.heap_at)($crate::wasm::data_end()) as usize
            }
            fn end() -> usize {
                $crate::wasm::given() as usize
            }
        }

        /// The module's one allocator (xpute_guest::mem::heap).
        #[global_allocator]
        static WASM_HEAP: $crate::mem::heap::Heap<WasmHeap> = $crate::mem::heap::Heap::new();

        /// What the module hands its guest (xpute_guest::entry `BootInfo`).
        pub fn boot_info() -> $crate::entry::BootInfo {
            $crate::wasm::boot_info(&$plan, || $crate::mem::heap::heap_stat(&WASM_HEAP))
        }

        /// The entry (xpute_guest::entry): a turn of `quota_ms`, and when the
        /// guest wants the next.
        #[no_mangle]
        pub extern "C" fn intr(quota_ms: f64) -> f64 {
            let entry: $crate::entry::Entry = $entry;
            entry(quota_ms, boot_info)
        }
    };
}
