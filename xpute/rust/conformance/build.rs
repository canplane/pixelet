// xpute-conformance/build.rs

// As wasm the memory is imported and the stack goes first, its size the
// core's own (spec/conformance/core.json), which cpp/Makefile matches.

#[path = "src/core.spec.rs"]
mod spec;

fn main() {
    xpute_core::wasm::link(&xpute_core::wasm::LinkConfig {
        stack_bytes: spec::STACK_BYTES,
        global_base: None,
        init_bytes: None,
    });
}
