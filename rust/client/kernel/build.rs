// pxl-kernel/build.rs

// The kernel linked as a WebAssembly module as every module guest of xpute
// is, by its machine (machine.json, xpute_guest::machine `link`): its memory
// imported, which the page reserves and gives it, its stack first, its data
// past the machine's front, and the smaller of the sizes a device may give it
// as the least it instantiates in, so a page that reserves less fails at
// once. Built for any other target it prints nothing, and the kernel is
// linked as the platform links any library.

#[path = "src/machine.spec.rs"]
mod machine;

use xpute_guest::wasm;

fn main() {
    wasm::link(&machine::MACHINE.link());
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/machine.spec.rs");
}
