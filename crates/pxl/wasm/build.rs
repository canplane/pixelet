// The module imports its memory: the central scheduler gives it one
// (@xpute/core/sys/memory.ts's reserve_memory), and the kernel lays out
// everything inside it (pxl_core::memory, over xpute_core::sys::memory).
// Declared no larger than its data needs and with no maximum, so whatever the
// central scheduler gives satisfies it.
//
// What is below is the front of that memory, and it is here rather than in the
// source because one half of it is a linker flag. The rings sit at addresses
// both sides compute rather than discover, which they can only do if the
// linker is kept out of the range they take — `--global-base` is how the
// linker is told where its own data may begin, and the same arithmetic has to
// produce that flag and the addresses the code uses, or the two drift apart
// silently and the linker writes `.data` over a ring. So the numbers are
// computed once here, emitted as the flag, and written out as a source file
// for the module to include.

use std::io::Write;

const PAGE: u32 = 1 << 12;

/// The stack, which `--stack-first` puts at the bottom so an overflow wraps
/// past zero and traps instead of writing over the data behind it. Its size is
/// the wasm32 target's own `-z stack-size`.
const STACK_BYTES: u32 = 1 << 20;

/// Messages a ring holds, and the bytes of the one frame each of them owns.
/// A packet is at most a frame; anything larger is bulk and crosses by
/// address. The frame is the kernel's page (pxl_core::memory::PAGE_BYTES),
/// which is also the buddy's smallest block, so a frame is one block wherever
/// one has to come from.
const RING_CAPACITY: u32 = 2048;
const RING_FRAME_BYTES: u32 = PAGE;

/// The descriptors: a header of 8 words and `RING_CAPACITY` messages of 4,
/// rounded up to a page so each ring begins on one.
const RING_DESC_BYTES: u32 = (8 + RING_CAPACITY * 4) * 4;
const RING_DESC_STRIDE: u32 = RING_DESC_BYTES.div_ceil(PAGE) * PAGE;

/// The frames, one a slot.
const RING_ARENA_STRIDE: u32 = RING_CAPACITY * RING_FRAME_BYTES;

/// Three rings: submission, completion, and the spill the completion ring
/// overflows into. Equal, because which way the traffic runs is a question
/// about the workload and the workload is still moving — the renderer came in
/// and what the shell still sends is on its way in behind it.
const RING_COUNT: u32 = 3;

const RING_DESC_AT: u32 = STACK_BYTES;
const RING_ARENA_AT: u32 = 2 << 20;

/// Where the linker may start putting `.rodata`, `.data` and `.bss`. A round
/// number rather than wherever the rings happen to end, so the two ranges grow
/// without moving each other: the rings have everything below it and the
/// module's own data has everything above it up to the terrain.
const GLOBAL_BASE: u32 = 32 << 20;

fn main() {
    const { assert!(RING_DESC_AT + RING_COUNT * RING_DESC_STRIDE <= RING_ARENA_AT, "the descriptors run into the frames") };
    const { assert!(RING_ARENA_AT + RING_COUNT * RING_ARENA_STRIDE <= GLOBAL_BASE, "the frames run into the module's data") };

    println!("cargo:rustc-link-arg=--import-memory");
    println!("cargo:rustc-link-arg=--global-base={GLOBAL_BASE}");
    println!("cargo:rerun-if-changed=build.rs");

    let out = std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("ring_layout.rs");
    let mut f = std::fs::File::create(&out).unwrap();
    write!(
        f,
        "\
/// Messages a ring holds. See build.rs.
pub const RING_CAPACITY: u32 = {RING_CAPACITY};
/// The bytes of the one frame each message owns: a packet is at most this.
pub const RING_FRAME_BYTES: u32 = {RING_FRAME_BYTES};
/// Where each ring's descriptors and frames are, both at addresses the shell
/// computes rather than reads.
pub const RING_DESC_AT: u32 = {RING_DESC_AT};
pub const RING_DESC_STRIDE: u32 = {RING_DESC_STRIDE};
pub const RING_ARENA_AT: u32 = {RING_ARENA_AT};
pub const RING_ARENA_STRIDE: u32 = {RING_ARENA_STRIDE};
/// Where the linker was told its own data may begin, which is what the rings
/// must stay below and what the module's data begins at.
pub const GLOBAL_BASE: u32 = {GLOBAL_BASE};
"
    )
    .unwrap();
}
