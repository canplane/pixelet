// pxl-core/build.rs

// Links the core as a WebAssembly module, the way xpute links every core
// built as one, from the core's manifest (manifest.json, xpute_core::manifest
// `link`). The module imports its memory, which the page reserves and hands
// it. The stack comes first in that memory, then the manifest's front (the
// header the runtime writes, and the GPU stream), then the module's data. The
// module's initial memory is the smallest size any device may give it, so a
// page that reserves less fails when it instantiates the module. For any
// other target the script prints nothing, and the platform links the core as
// it links any library.

#[path = "src/sys/manifest.spec.rs"]
mod manifest;

use xpute_core::wasm;

fn main() {
    let _ = manifest::GPU_ASK;
    wasm::link(&manifest::MANIFEST.link());
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/sys/manifest.spec.rs");
}
