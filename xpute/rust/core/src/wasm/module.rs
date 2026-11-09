// xpute-core/wasm/module.rs

//! This is what a module core reads of the module itself. It assumes the module
//! was linked as `link` links it.

extern "C" {
    /// The end of `.rodata`, `.data` and `.bss`, defined by wasm-ld. A core
    /// may begin placing its own ranges here. It is not `__heap_base`, which
    /// lies eight bytes further on: that is where a C runtime's `sbrk`
    /// starts, and a core places its heap where it says.
    #[allow(non_upper_case_globals)]
    static __data_end: u8;
    /// The top of the stack, defined by wasm-ld: `-z stack-size` bytes past
    /// the stack's start, which `--stack-first` puts at the bottom of the
    /// memory.
    #[allow(non_upper_case_globals)]
    static __stack_high: u8;
}

// `wasm_import_module` makes these imports of `env`, the module the runtime
// instantiates the core with. An extern block that names no import module
// fails to link.
#[link(wasm_import_module = "env")]
extern "C" {
    #[link_name = "now"]
    fn runtime_now() -> f64;
    #[link_name = "call"]
    fn runtime_call(at: u32) -> i32;
}

/// The stack's size, since the stack starts at address 0.
pub fn stack_high() -> u32 {
    core::ptr::addr_of!(__stack_high) as u32
}

pub fn data_end() -> u32 {
    core::ptr::addr_of!(__data_end) as u32
}

/// This is the size in bytes of the memory the runtime has given the module.
pub fn mem_sz() -> u64 {
    core::arch::wasm32::memory_size(0) as u64 * u64::from(super::WASM_PAGE_BYTES)
}

pub fn now() -> f64 {
    // SAFETY: an import with no arguments.
    unsafe { runtime_now() }
}

pub fn call(at: u32) -> i32 {
    // SAFETY: an import of one number; what it names, the runtime reads and
    // writes before it returns.
    unsafe { runtime_call(at) }
}
