// pxl-core/build.rs

// Links the core as a WebAssembly module, the way xpute links every core
// built as one, from the core's machine (machine.json, xpute_core::machine
// `link`). The module imports its memory, which the page reserves and hands
// it. The stack comes first in that memory, then the machine's front (the
// header and rings the runtime shares with the core), then the module's
// data. The module's initial memory is the smallest size any device may give
// it, so a page that reserves less fails when it instantiates the module.
// For any other target the script prints nothing, and the platform links the
// core as it links any library.

#[path = "src/sys/machine.spec.rs"]
mod machine;

use xpute_core::wasm;

fn main() {
    wasm::link(&machine::MACHINE.link());
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/sys/machine.spec.rs");
}
