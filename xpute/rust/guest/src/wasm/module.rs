// xpute-guest/wasm/module.rs

//! What a module guest reads of the module itself, linked as `link` links it.

extern "C" {
    /// The end of `.rodata`, `.data` and `.bss`, where a guest may begin
    /// placing its own ranges. wasm-ld defines it. Not `__heap_base`, eight
    /// bytes further on: a C runtime's `sbrk` starts there, and a guest's heap
    /// is where the guest says.
    #[allow(non_upper_case_globals)]
    static __data_end: u8;
    /// The top of the stack: `-z stack-size` past its start, which
    /// `--stack-first` puts at the bottom of the memory. wasm-ld defines it.
    #[allow(non_upper_case_globals)]
    static __stack_high: u8;
}

// `wasm_import_module` makes this an import of `env`, the module the host
// instantiates with; an extern block that names none is a link error.
#[link(wasm_import_module = "env")]
extern "C" {
    #[link_name = "now"]
    fn host_now() -> f64;
    #[link_name = "hostcall"]
    fn host_hostcall(at: u32) -> i32;
}

/// Its size, with the stack first.
pub fn stack_high() -> u32 {
    core::ptr::addr_of!(__stack_high) as u32
}

pub fn data_end() -> u32 {
    core::ptr::addr_of!(__data_end) as u32
}

/// The bytes of memory the host has grown the module to.
pub fn given() -> u64 {
    core::arch::wasm32::memory_size(0) as u64 * u64::from(super::WASM_PAGE_BYTES)
}

pub fn now() -> f64 {
    // SAFETY: an import with no arguments.
    unsafe { host_now() }
}

pub fn hostcall(at: u32) -> i32 {
    // SAFETY: an import of one number; what it names, the host reads and
    // writes before it returns.
    unsafe { host_hostcall(at) }
}
