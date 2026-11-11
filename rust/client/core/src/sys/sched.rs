// pxl-core/sys/sched.rs

//! How the core spends a turn: the one turn every task of the core runs
//! against (xpute_core::sched::turn), which decides when the turn's tasks
//! yield and whether the core asks for the next turn; the phases of its tick
//! and the share of the budget each holds back; and the wall clock time the
//! runtime tells it.

pub mod clock;
pub mod executor;
#[path = "sched/pacing.spec.rs"]
pub mod pacing_spec;
pub mod param;
#[path = "sched/phase.spec.rs"]
pub mod phase_spec;
pub mod tick;

use xpute_core::clock::now;
use xpute_core::global::Global;
use xpute_core::sched::turn::Turn;

use crate::sys::sched::pacing_spec::MARGIN_SHARE;
use crate::sys::sched::param::CONTENT_SHARE;

static TURN: Global<Turn> = Global::new();
/// How many turns have started.
static TURNS: Global<u64> = Global::new();

/// The one turn every task of the core runs against.
pub fn turn() -> &'static mut Turn {
    TURN.get(Turn::new)
}

/// Starts a turn that may last `deadline_ms` (`Turn::start`), and counts it.
pub fn start(deadline_ms: f64) {
    *TURNS.get(|| 0) += 1;
    turn().start(deadline_ms);
}

/// How many turns have started, which the profiler shows.
pub fn turns() -> u64 {
    *TURNS.get(|| 0)
}

/// Lets the turn's tasks run for their share of the turn after the frame,
/// however far the frame overran the deadline. The share is `CONTENT_SHARE`
/// of the frame time the runtime paces, less the runtime's margin, which the
/// deadline leaves out as well.
pub fn tasks_after_frame(paced_ms: f64) {
    turn().extend_to(now() + CONTENT_SHARE * paced_ms * (1.0 - MARGIN_SHARE));
}

/// Ends the turn and answers the ms until the core wants its next
/// (`Turn::end`). A task that is queued and not yet polled asks for the next
/// turn too, because a turn with no frame and no deadline left polls no task.
pub fn wake_ms() -> f64 {
    if executor::pending() {
        turn().ask_next();
    }
    turn().end()
}

pub fn request_world_tick() {
    turn().ask_next();
}

/// Asks for a turn at `at`. Of several asks the earliest wins, and a wait
/// that is still pending when its turn comes must ask again.
pub fn wake_at(at: f64) {
    turn().wake_at(at);
}
