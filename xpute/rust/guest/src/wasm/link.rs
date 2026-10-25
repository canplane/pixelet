// xpute-guest/wasm/link.rs

//! How a guest built as one WebAssembly module is linked, for its build
//! script, which runs on the build machine and so takes this with no cfg:
//!
//! ```
//! // a guest's build.rs, in its `main`
//! xpute_guest::wasm::link(&xpute_guest::wasm::LinkPlan { stack_bytes: 1 << 20, global_base: None, initial_bytes: None });
//! ```
//!
//! Cargo applies a `rustc-link-arg` only to the package whose script prints
//! it, so each guest calls `link` itself. Every module guest imports its
//! memory, and puts its stack first, so an overflow runs off address 0 and
//! traps rather than writing over the data above it. The C++ guest links
//! with the same flags (`cpp/Makefile` `WASM_LINK`).

/// A WebAssembly page, the unit a memory is declared in: 64 KiB, not the
/// 4 KiB page a guest places its ranges and its heap by
/// (`crate::mem::section::PAGE_BYTES`).
pub const WASM_PAGE_BYTES: u32 = 1 << 16;

/// A module guest's link numbers.
pub struct LinkPlan {
    /// The stack's size (`-z stack-size`), placed first (`--stack-first`).
    pub stack_bytes: u32,
    /// Where the linker's own data begins (`--global-base`), for a guest that
    /// keeps a front of its own below it.
    pub global_base: Option<u32>,
    /// The least memory the module instantiates in (`--initial-memory`), so a
    /// host that hands it less fails at once. Whole pages.
    pub initial_bytes: Option<u32>,
}

/// Prints `plan`'s link arguments, and nothing when the target is not
/// wasm32, where a guest is linked as the platform links any library.
pub fn link(plan: &LinkPlan) {
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() != Ok("wasm32") {
        return;
    }
    for arg in args(plan) {
        println!("cargo:rustc-link-arg={arg}");
    }
}

/// The arguments, in the order the linker is given them.
fn args(plan: &LinkPlan) -> Vec<String> {
    let mut args = vec!["--import-memory".to_string(), "--stack-first".to_string(), format!("-zstack-size={}", plan.stack_bytes)];
    if let Some(at) = plan.global_base {
        args.push(format!("--global-base={at}"));
    }
    if let Some(bytes) = plan.initial_bytes {
        assert!(bytes.is_multiple_of(WASM_PAGE_BYTES), "a memory is declared in WebAssembly pages, not {bytes} bytes");
        args.push(format!("--initial-memory={bytes}"));
    }
    args
}

#[cfg(test)]
#[path = "link.test.rs"]
mod test;
