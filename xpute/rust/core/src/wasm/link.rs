// xpute-core/wasm/link.rs

//! This is how a core built as one WebAssembly module is linked. A core's
//! build script calls `link`; a build script runs on the build machine, so
//! this module is compiled for every target:
//!
//! ```
//! // a core's build.rs, in its `main`
//! xpute_core::wasm::link(&xpute_core::wasm::LinkConfig { stack_bytes: 1 << 20, global_base: None, init_bytes: None });
//! ```
//!
//! Cargo applies a `rustc-link-arg` only to the package whose script prints
//! it, so each core calls `link` itself. Every module core imports its
//! memory and puts its stack first, so that a stack overflow runs below
//! address 0 and traps instead of writing over the data above the stack. The
//! C++ core links with the same flags (`cpp/Makefile` `WASM_LINK`).

/// A WebAssembly page, the unit a memory is declared in, is 64 KiB. It is not
/// the 4 KiB page by which a core places its ranges and its heap
/// (`crate::mem::section::PAGE_BYTES`).
pub const WASM_PAGE_BYTES: u32 = 1 << 16;

/// These are the numbers a module core is linked with.
pub struct LinkConfig {
    /// The stack's size (`-z stack-size`). The stack is placed first in the
    /// memory (`--stack-first`).
    pub stack_bytes: u32,
    /// Where the linker's own data begins (`--global-base`), for a core that
    /// keeps the front of the memory below its data (`crate::machine`).
    pub global_base: Option<u32>,
    /// The least memory the module can be instantiated with
    /// (`--initial-memory`), so that a runtime handing it less fails at once.
    /// It must be a whole number of WebAssembly pages.
    pub init_bytes: Option<u32>,
}

/// `link` prints `cfg`'s link arguments for Cargo. When the target is not
/// wasm32 it prints nothing, and the core is linked as the platform links any
/// library.
pub fn link(cfg: &LinkConfig) {
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() != Ok("wasm32") {
        return;
    }
    for arg in args(cfg) {
        println!("cargo:rustc-link-arg={arg}");
    }
}

fn args(cfg: &LinkConfig) -> Vec<String> {
    let mut args = vec![
        "--import-memory".to_string(),
        "--stack-first".to_string(),
        format!("-zstack-size={}", cfg.stack_bytes),
    ];
    if let Some(at) = cfg.global_base {
        args.push(format!("--global-base={at}"));
    }
    if let Some(bytes) = cfg.init_bytes {
        assert!(
            bytes.is_multiple_of(WASM_PAGE_BYTES),
            "a memory is declared in WebAssembly pages, not {bytes} bytes"
        );
        args.push(format!("--initial-memory={bytes}"));
    }
    args
}

#[cfg(test)]
#[path = "link.test.rs"]
mod test;
