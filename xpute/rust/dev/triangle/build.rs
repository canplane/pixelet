// xpute-triangle/build.rs

// The module is linked as its machine says. For a target other than
// WebAssembly this prints nothing, and the platform links the core as any
// library.

#[path = "src/machine.spec.rs"]
mod machine;

fn main() {
    let _ = machine::GPU_ASK;
    xpute_core::wasm::link(&machine::MACHINE.link());
}
