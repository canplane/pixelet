// xpute-conformance/build.rs

// As wasm the memory is imported and the stack goes first, its size the
// guest's own (spec/conformance/guest.json), which cpp/Makefile matches.

#[path = "src/guest.spec.rs"]
mod spec;

fn main() {
    xpute_guest::wasm::link(&xpute_guest::wasm::LinkConfig {
        stack_bytes: spec::STACK_BYTES,
        global_base: None,
        init_bytes: None,
    });
}
