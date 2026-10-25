// xpute-conformance/build.rs

// As wasm the memory is imported and the stack goes first, xpute-link's 1 MiB
// (`STACK_BYTES`), which the C++ build (cpp/Makefile) matches.

fn main() {
    xpute_link::module(&xpute_link::Module::default());
}
