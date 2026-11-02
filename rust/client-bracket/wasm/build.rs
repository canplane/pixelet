// pxl-wasm/build.rs

// The module is linked as every module guest of xpute is (xpute_guest::wasm
// `link`): its memory imported, which the central scheduler gives it
// (@xpute/host/mem/reserve.ts's reserve_mem), and its stack first. The
// kernel lays out everything inside that memory (pxl_kernel::sys::mem, over
// xpute_guest::mem).
//
// The front of that memory is the door's (pxl-kernel door/layout.rs): the
// rings sit at addresses both sides compute, outside the linker's range.
// `--global-base`, where the linker's own data may begin, is mem.json's
// GLOBAL_BASE, the number layout.rs asserts the rings end below.
//
// The numbers are mem.json's, which pxl-tool/bin/spec.rs writes beside this
// script as it writes the kernel's sys/mem/mem.spec.rs.

#[path = "mem.spec.rs"]
mod spec;

use spec::{DESKTOP_BYTES, GLOBAL_BASE, PHONE_BYTES, STACK_BYTES};
use xpute_guest::wasm;
use xpute_guest::wasm::LinkConfig;

/// The memory the module declares it needs, which the engine checks when the
/// shell hands it one: the smallest reservation this build can run in. A shell
/// that asks for less fails to instantiate.
const MIN_MEM_BYTES: u32 = PHONE_BYTES;

fn main() {
    const {
        assert!(
            MIN_MEM_BYTES <= DESKTOP_BYTES,
            "the module asks for more than the larger of the two reservations"
        );
    };

    wasm::link(&LinkConfig {
        stack_bytes: STACK_BYTES,
        global_base: Some(GLOBAL_BASE),
        init_bytes: Some(MIN_MEM_BYTES),
    });
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=mem.spec.rs");
}
