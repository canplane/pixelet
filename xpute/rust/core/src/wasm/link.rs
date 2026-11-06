// xpute-core/wasm/link.rs

//! How a core built as one WebAssembly module is linked, for its build
//! script, which runs on the build machine and so takes this with no config:
//!
//! ```
//! // a core's build.rs, in its `main`
//! xpute_core::wasm::link(&xpute_core::wasm::LinkConfig { stack_bytes: 1 << 20, global_base: None, init_bytes: None });
//! ```
//!
//! Cargo applies a `rustc-link-arg` only to the package whose script prints
//! it, so each core calls `link` itself. Every module core imports its
//! memory, and puts its stack first, so an overflow runs off address 0 and
//! traps rather than writing over the data above it. The C++ core links
//! with the same flags (`cpp/Makefile` `WASM_LINK`).

/// A WebAssembly page, the unit a memory is declared in: 64 KiB, not the
/// 4 KiB page a core places its ranges and its heap by
/// (`crate::mem::section::PAGE_BYTES`).
pub const WASM_PAGE_BYTES: u32 = 1 << 16;

/// A module core's link numbers.
pub struct LinkConfig {
    /// The stack's size (`-z stack-size`), placed first (`--stack-first`).
    pub stack_bytes: u32,
    /// Where the linker's own data begins (`--global-base`), for a core that
    /// keeps a front of its own below it.
    pub global_base: Option<u32>,
    /// The least memory the module instantiates in (`--initial-memory`), so a
    /// runtime that hands it less fails at once. Whole pages.
    pub init_bytes: Option<u32>,
}

/// Prints `cfg`'s link arguments, and nothing when the target is not
/// wasm32, where a core is linked as the platform links any library.
pub fn link(cfg: &LinkConfig) {
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() != Ok("wasm32") {
        return;
    }
    for arg in args(cfg) {
        println!("cargo:rustc-link-arg={arg}");
    }
}

/// The arguments, in the order the linker is given them.
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
