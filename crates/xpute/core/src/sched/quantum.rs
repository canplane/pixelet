// xpute_core/sched/quantum.rs

//! The central scheduler's grant: the outermost of the scheduler's three
//! levels.
//!
//! The central scheduler runs guests the way an OS runs processes: it owns
//! the clock and the resources, hands a guest the turn with a grant, and
//! takes back what the guest spent. A guest's own scheduler (sched/tick.rs)
//! spends the grant in its phases, and a phase spends its share in passes
//! (sched/frame_budget.rs). Memory is granted the same way, once
//! (sys/mem.rs, sys/slab.ts), and I/O by credits (sched/io.rs).
//!
//! The grant is sized from what only the central scheduler sees: the
//! display's frame interval, and what a turn costs outside the guest's slice
//! — the host's own work around the call, what the guest does outside its
//! scheduler. The guest sees neither, and does not size its own slice.
//!
//! **Two quanta, and the guest picks.** An interactive guest gets what is
//! left of the frame; a guest that has held still gets the batch quantum,
//! past a frame on purpose: the frame exists to keep motion smooth, and with
//! nothing moving a frame that takes two vsyncs shows the same image twice.
//! That is the split a multilevel feedback queue makes between interactive
//! and batch work. Both are granted up front because which applies is known
//! only partway into the guest's turn — its interaction phase says whether
//! the view moved — and the frame a gesture starts on must already be sized
//! for motion.
//!
//! **The frame interval is measured, not assumed.** `1000 / 60` is wrong on
//! any faster display: a 5.8 ms frame was once sized for 16.7, and a storm
//! spent more than a whole frame scheduling. It is a low percentile of recent
//! gaps, not their mean — the mean includes the frames the guest itself made
//! long, which would feed an overrun back into a bigger slice. Only gaps
//! between two interactive turns count, and the percentile is the 25th: a
//! held-still turn runs long on purpose, and the host answers a late frame by
//! firing the next one almost at once — gaps that once read as a "2.1 ms
//! display" on a 170 Hz screen and pinned every moving frame to the floor.
//! Gaps past MAX_FRAME_GAP_MS are not frames at all: an idle guest asks for
//! none, and the gap before the next can be seconds.
//!
//! Every number that decides how much is the guest's policy (QuantumPolicy),
//! declared to the central scheduler, not chosen here.

pub const FRAME_WINDOW: usize = 64;
const MAX_FRAME_GAP_MS: f64 = 50.0;
/// The cost outside the slice before any turn has been measured.
const OUTSIDE_GUESS_MS: f64 = 4.0;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct QuantumPolicy {
    /// What is left of the frame unspent, as a share of it: for the host's
    /// collector and the turns the estimate is wrong about.
    pub margin_share: f64,
    /// The interactive slice's clamp: the floor keeps a starved frame making
    /// progress, the ceiling keeps a cheap turn from growing into the frame.
    pub min_ms: f64,
    pub max_ms: f64,
    /// A held-still guest's quantum.
    pub batch_ms: f64,
    /// Smoothing on the cost outside the slice.
    pub outside_ema: f64,
}

/// About how long the guest should take this turn. A hint, not a deadline:
/// nothing is preempted, a step runs to completion, and a turn that runs past
/// it costs the next grant what it overran.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Grant {
    /// While interactive.
    pub slice_ms: f64,
    /// Once held still.
    pub batch_ms: f64,
    /// The display's measured frame interval the slice was sized from.
    pub frame_ms: f64,
}

/// What a guest's turn took, as the central scheduler saw it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Turn {
    /// The gap before the turn, in seconds.
    pub delta_s: f64,
    /// The whole turn, around the call.
    pub turn_ms: f64,
    /// What the guest's scheduler spent of its grant, as it reports.
    pub used_ms: f64,
    /// Whether the guest ran on the interactive slice.
    pub interactive: bool,
}

pub struct Quantum {
    policy: QuantumPolicy,
    gaps: [f64; FRAME_WINDOW],
    gap_at: usize,
    outside_ms: f64,
    prev_interactive: bool,
}

impl Quantum {
    pub fn new(policy: QuantumPolicy) -> Quantum {
        Quantum {
            policy,
            gaps: [1000.0 / 60.0; FRAME_WINDOW],
            gap_at: 0,
            outside_ms: OUTSIDE_GUESS_MS,
            prev_interactive: false,
        }
    }

    /// Learns from a turn that finished.
    pub fn observe(&mut self, turn: Turn) {
        let gap_ms = turn.delta_s * 1000.0;
        if turn.interactive && self.prev_interactive && gap_ms > 0.0 && gap_ms <= MAX_FRAME_GAP_MS {
            self.gaps[self.gap_at] = gap_ms;
            self.gap_at = (self.gap_at + 1) % FRAME_WINDOW;
        }
        self.prev_interactive = turn.interactive;
        let outside = (turn.turn_ms - turn.used_ms).max(0.0);
        self.outside_ms += (outside - self.outside_ms) * self.policy.outside_ema;
    }

    /// The grant for the next turn.
    pub fn grant(&self) -> Grant {
        let mut gaps = self.gaps;
        gaps.sort_unstable_by(f64::total_cmp);
        let frame = gaps[FRAME_WINDOW / 4];
        let p = self.policy;
        Grant {
            slice_ms: p.max_ms.min(p.min_ms.max(frame - frame * p.margin_share - self.outside_ms)),
            batch_ms: p.batch_ms,
            frame_ms: frame,
        }
    }
}

#[cfg(test)]
#[path = "quantum.test.rs"]
mod test;
