// xpute-example/build.rs

// As a module the memory is imported and the stack goes first, its size the
// one `wasm_abi!` checks as it boots (src/lib.rs `STACK_BYTES`).

fn main() {
    xpute_guest::wasm::link(&xpute_guest::wasm::LinkConfig {
        stack_bytes: 1 << 16,
        global_base: None,
        init_bytes: None,
    });
}
