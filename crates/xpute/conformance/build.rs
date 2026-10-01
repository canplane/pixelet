// xpute-conformance/build.rs

// The guest built as a WebAssembly module: the memory is the host's, imported
// rather than defined, as a page's is, and the stack is said here rather than
// left to the target's default. What lies past the stack and the linker's
// data is the guest's range (guest.rs), so neither number reaches a host; the
// stack is 1 MiB, the target's own default written down so that the C++
// build (script/cpp.ts) can say the same.

const STACK_BYTES: u32 = 1 << 20;

fn main() {
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32") {
        println!("cargo:rustc-link-arg=--import-memory");
        println!("cargo:rustc-link-arg=--stack-first");
        println!("cargo:rustc-link-arg=-zstack-size={STACK_BYTES}");
    }
}
