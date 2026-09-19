// The module imports its memory: the central scheduler gives it one
// (@xpute/runtime/sys/mem.ts's reserve_memory), and the kernel lays out
// everything inside it (pxl_kernel::mem, over xpute_runtime::mem).
//
// What is below is the front of that memory, and it is here rather than in the
// source because half of it is linker flags. The rings sit at addresses both
// sides compute rather than discover, which they can only do if the linker is
// kept out of the range they take — `--global-base` is how the linker is told
// where its own data may begin, and the same numbers have to produce that flag
// and the addresses the code uses, or the two drift apart silently and the
// linker writes `.data` over a ring.
//
// The numbers themselves are not decided here either. They are mem.json's,
// the same file the map and the shell read, included below rather than copied
// (script/spec.ts writes mem/spec.rs; `deno task check:spec` fails where
// what is on disk is not what the JSON writes).

use std::io::Write;

#[path = "../kernel/src/mem/spec.rs"]
mod spec;

use spec::{DESKTOP_BYTES, GLOBAL_BASE, PAGE_BYTES, PHONE_BYTES, RING_CAPACITY, RING_DESC_AT, RING_SLOT_AT, STACK_BYTES};

/// A ring slot's payload is a page: a packet is at most one, and anything
/// larger is bulk and crosses by address. The page is also the buddy's
/// smallest block, so a payload is one block wherever one has to come from.
const RING_SLOT_BYTES: u32 = PAGE_BYTES;

/// The descriptors: a header of 8 words and `RING_CAPACITY` messages of 4,
/// rounded up to a page so each ring begins on one.
const RING_DESC_BYTES: u32 = (8 + RING_CAPACITY * 4) * 4;
const RING_DESC_STRIDE: u32 = RING_DESC_BYTES.div_ceil(PAGE_BYTES) * PAGE_BYTES;

/// The payloads, one a slot.
const RING_SLOT_STRIDE: u32 = RING_CAPACITY * RING_SLOT_BYTES;

/// Three rings: submission, completion, and the spill the completion ring
/// overflows into. Equal, because which way the traffic runs is a question
/// about the workload and the workload is still moving — the renderer came in
/// and what the shell still sends is on its way in behind it.
const RING_COUNT: u32 = 3;

/// The memory the module declares it needs, which the engine checks when the
/// shell hands it one: the smallest reservation this build can run in. A shell
/// that asks for less fails to instantiate, where before it instantiated fine
/// and the kernel asserted its way out of it a moment later.
const MIN_MEMORY_BYTES: u32 = PHONE_BYTES;

fn main() {
    // Every boundary here is a chosen constant rather than a rounding, so what
    // a `page_up` would have guaranteed has to be asserted instead.
    const {
        assert!(
            RING_DESC_AT.is_multiple_of(PAGE_BYTES) && RING_SLOT_AT.is_multiple_of(PAGE_BYTES) && GLOBAL_BASE.is_multiple_of(PAGE_BYTES),
            "a range begins on a page, so every slot in it begins on one"
        )
    };
    const { assert!(STACK_BYTES <= RING_DESC_AT, "the stack runs into the descriptors") };
    const { assert!(RING_DESC_AT + RING_COUNT * RING_DESC_STRIDE <= RING_SLOT_AT, "the descriptors run into the payloads") };
    const { assert!(RING_SLOT_AT + RING_COUNT * RING_SLOT_STRIDE <= GLOBAL_BASE, "the payloads run into the module's data") };
    const { assert!(MIN_MEMORY_BYTES <= DESKTOP_BYTES, "the module asks for more than the larger of the two reservations") };
    const { assert!(MIN_MEMORY_BYTES.is_multiple_of(1 << 16), "a memory is declared in WebAssembly pages") };

    println!("cargo:rustc-link-arg=--import-memory");
    println!("cargo:rustc-link-arg=--stack-first");
    println!("cargo:rustc-link-arg=-zstack-size={STACK_BYTES}");
    println!("cargo:rustc-link-arg=--global-base={GLOBAL_BASE}");
    println!("cargo:rustc-link-arg=--initial-memory={MIN_MEMORY_BYTES}");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../kernel/src/mem/spec.rs");

    let out = std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("ring_layout.rs");
    let mut f = std::fs::File::create(&out).unwrap();
    write!(
        f,
        "\
/// Messages a ring holds. See build.rs and mem.json.
pub const RING_CAPACITY: u32 = {RING_CAPACITY};
/// The bytes of the payload each slot owns: a packet is at most this.
pub const RING_SLOT_BYTES: u32 = {RING_SLOT_BYTES};
/// Where each ring's descriptors and payloads are, at addresses that are the
/// same in every build rather than wherever something handed them out.
pub const RING_DESC_AT: u32 = {RING_DESC_AT};
pub const RING_DESC_STRIDE: u32 = {RING_DESC_STRIDE};
pub const RING_SLOT_AT: u32 = {RING_SLOT_AT};
pub const RING_SLOT_STRIDE: u32 = {RING_SLOT_STRIDE};
"
    )
    .unwrap();
}
