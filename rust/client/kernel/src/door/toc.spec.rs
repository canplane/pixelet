// pxl-kernel/door/toc.spec.rs
//
// GENERATED from spec/client/kernel/door/toc.json
//
// The kernel's own entries in its toc (xpute-guest ipc/toc.rs), each a
// key with xpute's PROGRAM bit set: the memory the host reads besides the rings and
// the hostcalls (pxl-kernel door.rs, ts/client/src/abi/boot.ts).

// Names as the spec spells them, so one record has one name on every side.
// A table is generated whole, whether or not this crate calls an entry.
#![allow(
    non_upper_case_globals,
    non_snake_case,
    clippy::absolute_paths,
    dead_code,
    reason = "generated from the spec"
)]

/// What an entry's value is.
pub mod key {
    /// Where the renderer's stream is, run after every turn; 0 where the kernel draws itself.
    pub const GPU_STREAM: u32 = 2147483650;
}
