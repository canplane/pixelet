// xpute-core/sched/frame.rs

//! What a core and a runtime that begins the core's turns on the core's own
//! thread tell each other of the display (xpute-runtime native/thread.rs). The
//! runtime gives each turn its frame, and the core sets how turns are paced. A
//! page's runtime grants frames through the rings instead, so a core there is
//! given no frame and has nowhere to tell its pace.

use crate::global::Global;

/// How turns are paced to the display, as @xpute/runtime sched/frame_pacing.ts
/// `FramePacingConfig` paces them on a page, and `asked_ms`, the least interval
/// the core asks between turns, 0 for none.
#[derive(Clone, Copy)]
pub struct Pace {
    pub pace_hz: f64,
    pub margin_share: f64,
    pub batch_frames: f64,
    pub settle_ms: f64,
    pub asked_ms: f64,
}

/// The display frame a turn was begun at.
#[derive(Clone, Copy)]
pub struct Frame {
    pub unix_ms: f64,
    pub refresh_ms: f64,
    /// The interval turns are paced at, a whole number of refreshes.
    pub paced_ms: f64,
    pub interactive: bool,
}

static FRAME: Global<Option<Frame>> = Global::new();
static PACE: Global<Option<fn(Pace)>> = Global::new();

/// Gives the next turn its frame.
pub fn give(frame: Frame) {
    FRAME.set(Some(frame));
}

/// The frame this turn was begun at, once. A turn rung, and any turn on a page,
/// has none.
pub fn take() -> Option<Frame> {
    FRAME.get(|| None).take()
}

pub(crate) fn on_pace(pace: Option<fn(Pace)>) {
    PACE.set(pace);
}

/// Sets how the turns are paced, and says whether a runtime took it.
pub fn pace(pace: Pace) -> bool {
    match *PACE.get(|| None) {
        Some(set) => {
            set(pace);
            true
        }
        None => false,
    }
}
