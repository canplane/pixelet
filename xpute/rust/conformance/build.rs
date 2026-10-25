// xpute-conformance/build.rs

// As wasm the memory is imported and the stack goes first, its size the
// guest's own (spec/conformance/guest.json), which cpp/Makefile matches.

#[path = "src/guest.spec.rs"]
mod spec;

fn main() {
    xpute_link::module(&xpute_link::Module {
        stack_bytes: spec::STACK_BYTES,
        global_base: None,
        initial_bytes: None,
    });
}
