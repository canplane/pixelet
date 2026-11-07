// pxl-kernel/sys/sched/pacing.spec.rs
//
// GENERATED from spec/client/kernel/sys/sched/pacing.json
//
// The numbers of the scheduling policy pixelet boots xpute's runtime with on a page or a
// device (@xpute/runtime sched/frame_pacing.ts): turns paced to the display, each given
// what its frames leave. The client's runtime injects them (ts/client/src/runtime/start.ts),
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

/// The rate turns are paced at where the kernel asks none slower: the most whole refreshes
/// of the display that fit one interval at this rate, one at least, so a 120 Hz display
/// runs a turn every second refresh.
pub const PACE_HZ: f64 = 60.0;

/// The slowest rate the kernel's load asks (pxl-kernel domain/scene/resolution.rs): past
/// it the pixels give way, not the motion.
pub const RATE_MIN_HZ: f64 = 30.0;

/// Share of the frame left unspent for the runtime's own work; a share, so a fast display
/// does not lose most of its frame to it.
pub const MARGIN_SHARE: f64 = 0.15;

/// Frames a non-interactive turn may take: with nothing moving, two vsyncs only show the same image twice.
pub const BATCH_FRAMES: f64 = 2.0;

/// How long after the last input a turn is still interactive, so a pause mid-drag does not count.
pub const SETTLE_MS: f64 = 150.0;
