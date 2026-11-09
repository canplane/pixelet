// xpute-conformance/build.rs

// Built as wasm, the core imports its memory and puts its stack first. The
// stack's size is the conformance core's own (spec/conformance/core.json),
// and cpp/Makefile links the C++ core with the same size.

#[path = "src/core.spec.rs"]
mod spec;

fn main() {
    xpute_core::wasm::link(&xpute_core::wasm::LinkConfig {
        stack_bytes: spec::STACK_BYTES,
        global_base: None,
        init_bytes: None,
    });
}
