// xpute-link/lib.rs

//! How a guest built as one WebAssembly module is linked, for its build
//! script:
//!
//! ```
//! // a guest's build.rs, in its `main`
//! xpute_link::module(&xpute_link::Module { stack_bytes: 1 << 20, global_base: None, initial_bytes: None });
//! ```
//!
//! Cargo applies a `rustc-link-arg` only to the package whose script
//! prints it, so each guest's `build.rs` calls `module` with its own numbers.
//!
//! What is the same for every module guest: the host hands it its memory, so
//! the memory is imported, never the module's own; and the stack goes first,
//! at the bottom of the memory, so an overflow runs off address 0 and traps
//! rather than writing over the data above it. The rest is the guest's: the
//! stack's size, and, for a guest that keeps ranges of its own below the
//! linker's data, where that data begins and the least memory it runs in.
//!
//! The C++ guest links with the same flags from its Makefile
//! (`cpp/Makefile` `WASM_LINK`), which a Rust crate cannot reach.

/// A WebAssembly page, the unit a memory is declared in: 64 KiB, not the
/// 4 KiB page a guest places its ranges and its heap by
/// (`xpute_guest::mem::section::PAGE_BYTES`).
pub const WASM_PAGE_BYTES: u32 = 1 << 16;

/// A module guest's numbers.
pub struct Module {
    /// The stack's size (`-z stack-size`), placed first (`--stack-first`).
    pub stack_bytes: u32,
    /// Where the linker's own data begins (`--global-base`), for a guest that
    /// keeps a front of its own below it.
    pub global_base: Option<u32>,
    /// The least memory the module instantiates in (`--initial-memory`), so a
    /// host that hands it less fails at once. Whole pages.
    pub initial_bytes: Option<u32>,
}

/// Prints `m`'s link arguments, and nothing when the target is not wasm32,
/// where a guest is linked as the platform links any library.
pub fn module(m: &Module) {
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() != Ok("wasm32") {
        return;
    }
    for arg in args(m) {
        println!("cargo:rustc-link-arg={arg}");
    }
}

/// The arguments, in the order the linker is given them.
pub fn args(m: &Module) -> Vec<String> {
    let mut args = vec!["--import-memory".to_string(), "--stack-first".to_string(), format!("-zstack-size={}", m.stack_bytes)];
    if let Some(at) = m.global_base {
        args.push(format!("--global-base={at}"));
    }
    if let Some(bytes) = m.initial_bytes {
        assert!(bytes.is_multiple_of(WASM_PAGE_BYTES), "a memory is declared in WebAssembly pages, not {bytes} bytes");
        args.push(format!("--initial-memory={bytes}"));
    }
    args
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod test;
