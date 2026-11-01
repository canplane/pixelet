// pxl-kernel/sys/sched/pacing.spec.rs
//
// GENERATED from spec/client/kernel/sys/sched/pacing.json
//
// The numbers of the scheduling policy pixelet boots xpute's host with on a page or a
// device (@xpute/host sched/frame_pacing.ts): turns paced to the display, each given
// what its frames leave. The client's boot injects them (ts/client/src/abi/boot.ts),
// and the kernel reads the margin to size its tasks past a full frame
// (pxl-kernel sys/sched/edge.rs).

// Names as the spec spells them, so one record has one name on every side.
// A table is generated whole, whether or not this crate calls an entry.
#![allow(
    non_upper_case_globals,
    non_snake_case,
    clippy::absolute_paths,
    dead_code,
    reason = "generated from the spec"
)]

/// Share of the frame left unspent for the host's own work; a share, so a fast display
/// does not lose most of its frame to it.
pub const MARGIN_SHARE: f64 = 0.15;

/// Frames a non-interactive turn may take: with nothing moving, two vsyncs only show the same image twice.
pub const BATCH_FRAMES: f64 = 2.0;

/// How long after the last input a turn is still interactive, so a pause mid-drag does not count.
pub const SETTLE_MS: f64 = 150.0;
