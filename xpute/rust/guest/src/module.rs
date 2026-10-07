// xpute-guest/module.rs

//! What a guest built as one WebAssembly module reads of the module itself:
//! where the linker stopped, the memory it was given, and the host's clock.

extern "C" {
    /// The end of `.rodata`, `.data` and `.bss`, where a guest may begin
    /// placing its own ranges. wasm-ld defines it. Not `__heap_base`, eight
    /// bytes further on: a C runtime's `sbrk` starts there, and a guest's heap
    /// is where the guest says.
    #[allow(non_upper_case_globals)]
    static __data_end: u8;
}

// `wasm_import_module` makes this an import of `env`, the module the host
// instantiates with; an extern block that names none is a link error.
#[link(wasm_import_module = "env")]
extern "C" {
    #[link_name = "now"]
    fn host_now() -> f64;
}

/// Where the linker stopped, as an address.
pub fn data_end() -> u32 {
    core::ptr::addr_of!(__data_end) as u32
}

/// The bytes of memory the host has grown the module to.
pub fn given() -> u64 {
    core::arch::wasm32::memory_size(0) as u64 * (1 << 16)
}

/// The host's clock, in milliseconds: what a turn's quota is spent against.
pub fn now() -> f64 {
    // SAFETY: an import with no arguments.
    unsafe { host_now() }
}
