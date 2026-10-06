// xpute-conformance/build.rs

// As wasm the memory is imported. The stack is the target's default 1 MiB,
// written down so the C++ build (script/cpp.ts) can match it.

const STACK_BYTES: u32 = 1 << 20;

fn main() {
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32") {
        println!("cargo:rustc-link-arg=--import-memory");
        println!("cargo:rustc-link-arg=--stack-first");
        println!("cargo:rustc-link-arg=-zstack-size={STACK_BYTES}");
    }
}
