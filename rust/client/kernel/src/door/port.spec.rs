// pxl-kernel/door/port.spec.rs
//
// GENERATED from spec/client/kernel/door/port.json
//
// pixelet's ports on xpute's rings (xpute spec/ipc/frame.json PORT_SHIFT): the number a
// frame names at the far side, the host being 0. 1 and 255 are the core memory's own,
// and 2 to 254 are kept for workers, each lent its slabs for a job. The client sends as
// the shell, and the kernel raises its events for it (pxl-kernel door.rs,
// ts/client/src/abi/boot.ts).

// Names as the spec spells them, so one record has one name on every side.
// A table is generated whole, whether or not this crate calls an entry.
#![allow(
    non_upper_case_globals,
    non_snake_case,
    clippy::absolute_paths,
    dead_code,
    reason = "generated from the spec"
)]

/// The core, the guest that holds the core memory: what a worker names to reach it.
pub const CORE: u32 = 1;

/// The shell, with the drivers that bring the outside in (ts/client/src/port/,
/// ts/client/src/driver/).
pub const SHELL: u32 = 255;
