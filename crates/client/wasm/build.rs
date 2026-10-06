// pxl-wasm/build.rs

// The module imports its memory: the central scheduler gives it one
// (@xpute/host/mem/reserve.ts's reserve_memory), and the kernel lays out
// everything inside it (pxl_kernel::mem, over xpute_guest::mem).
//
// The front of that memory is the door's (pxl-kernel door/layout.rs): the
// rings sit at addresses both sides compute rather than discover, which they
// can only do if the linker is kept out of the range they take.
// `--global-base` is how the linker is told where its own data may begin, and
// it is mem.json's GLOBAL_BASE, the same number layout.rs asserts the rings
// end below, or the two would drift apart silently and the linker would write
// `.data` over a ring.
//
// The numbers are not decided here either. They are mem.json's, included
// below rather than copied (pxl-tool/bin/spec.rs writes mem/mem.spec.rs;
// `deno task check:spec` fails where what is on disk is not what the JSON
// writes).

#[path = "../kernel/src/mem/mem.spec.rs"]
mod spec;

use spec::{DESKTOP_BYTES, GLOBAL_BASE, PHONE_BYTES, STACK_BYTES};

/// The memory the module declares it needs, which the engine checks when the
/// shell hands it one: the smallest reservation this build can run in. A shell
/// that asks for less fails to instantiate, where before it instantiated fine
/// and the kernel asserted its way out of it a moment later.
const MIN_MEMORY_BYTES: u32 = PHONE_BYTES;

fn main() {
    const { assert!(MIN_MEMORY_BYTES <= DESKTOP_BYTES, "the module asks for more than the larger of the two reservations") };
    const { assert!(MIN_MEMORY_BYTES.is_multiple_of(1 << 16), "a memory is declared in WebAssembly pages") };

    println!("cargo:rustc-link-arg=--import-memory");
    println!("cargo:rustc-link-arg=--stack-first");
    println!("cargo:rustc-link-arg=-zstack-size={STACK_BYTES}");
    println!("cargo:rustc-link-arg=--global-base={GLOBAL_BASE}");
    println!("cargo:rustc-link-arg=--initial-memory={MIN_MEMORY_BYTES}");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../kernel/src/mem/mem.spec.rs");
}
