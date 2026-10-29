// pxl-wasm/build.rs

// The module is linked as every module guest of xpute is (xpute_guest::wasm
// `link`): its memory imported, which the central scheduler gives it
// (@xpute/host/mem/reserve.ts's reserve_memory), and its stack first. The
// kernel lays out everything inside that memory (pxl_kernel::sys::mem, over
// xpute_guest::mem).
//
// The front of that memory is the door's (pxl-kernel door/layout.rs): the
// rings sit at addresses both sides compute rather than discover, which they
// can only do if the linker is kept out of the range they take.
// `--global-base` is how the linker is told where its own data may begin, and
// it is mem.json's GLOBAL_BASE, the same number layout.rs asserts the rings
// end below, or the two would drift apart silently and the linker would write
// `.data` over a ring.
//
// The numbers are not decided here either. They are mem.json's, which
// pxl-tool/bin/spec.rs writes beside this script as it writes the kernel's
// sys/mem/mem.spec.rs (`make check-spec` fails where what is on disk is not
// what the JSON writes).

#[path = "mem.spec.rs"]
mod spec;

use spec::{DESKTOP_BYTES, GLOBAL_BASE, PHONE_BYTES, STACK_BYTES};

/// The memory the module declares it needs, which the engine checks when the
/// shell hands it one: the smallest reservation this build can run in. A shell
/// that asks for less fails to instantiate, where before it instantiated fine
/// and the kernel asserted its way out of it a moment later.
const MIN_MEM_BYTES: u32 = PHONE_BYTES;

fn main() {
    const {
        assert!(
            MIN_MEM_BYTES <= DESKTOP_BYTES,
            "the module asks for more than the larger of the two reservations"
        )
    };

    xpute_guest::wasm::link(&xpute_guest::wasm::LinkPlan {
        stack_bytes: STACK_BYTES,
        global_base: Some(GLOBAL_BASE),
        initial_bytes: Some(MIN_MEM_BYTES),
    });
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=mem.spec.rs");
}
