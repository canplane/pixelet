// pxl-core/build.rs

// Links the core as a WebAssembly module from its manifest (manifest.json):
// the module owns its memory and grows it, and its stack comes first in it,
// so that an overflow runs below address 0 and traps instead of writing over
// the data above the stack. For any other target the script prints nothing,
// and the platform links the core as it links any library.

#[path = "src/spec/manifest.spec.rs"]
pub mod manifest;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/spec/manifest.spec.rs");
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() != Ok("wasm32") {
        return;
    }
    println!("cargo:rustc-link-arg=--stack-first");
    println!("cargo:rustc-link-arg=-zstack-size={}", manifest::stack::MODULE_BYTES);
}
