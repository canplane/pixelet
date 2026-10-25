// xpute-conformance/build.rs

// As wasm the memory is imported and the stack goes first (xpute-link). The
// stack is the target's default 1 MiB, written down so the C++ build
// (cpp/Makefile) can match it.

const STACK_BYTES: u32 = 1 << 20;

fn main() {
    xpute_link::module(&xpute_link::Module {
        stack_bytes: STACK_BYTES,
        global_base: None,
        initial_bytes: None,
    });
}
