// xpute-runtime/sched/quantum.rs

//! The central scheduler's grant: the outermost of the scheduler's three
//! levels.
//!
//! The central scheduler runs guests the way an OS runs processes: it owns
//! the clock and the resources, and the turn alternates like a clock's two
//! edges. On the rising edge the host hands the guest one quota, the time it
//! may take; the guest runs its own tasks cooperatively (sched/tick.rs) and,
//! on the falling edge, hands the turn back with when it wants the next one.
//! The host knows nothing of what runs inside. Memory is granted once
//! (sys/mem.rs, sys/slab.ts), and I/O by credits (sched/io.rs).
//!
//! **One quota, sized from what the host sees.** Nothing is preempted, so a
//! turn may run past its quota by the step that noticed; what it overran
//! comes off its own next quota, down to nothing, and the host's share of
//! the frame is left as it was (a constant bandwidth server's rule). Whether
//! the turn is interactive is the host's call, as an OS scheduler's is, from
//! behaviour rather than from the guest's inside: input within `settle_ms`,
//! or a guest that asked for the very next edge. An interactive turn gets
//! what the frame leaves after the margin; otherwise that of `batch_frames`
//! frames, past one on purpose — with nothing moving, a frame that takes two
//! vsyncs shows the same image twice.
//!
//! **No number here is a measured cost.** The quota is a share of the frame
//! the display sets; how much work fits in it is the guest's to find out by
//! yielding, and an overrun corrects itself through the next quota. The floor
//! is not a number of milliseconds either: a pass always takes its first step
//! (sched/frame_budget.rs), so a quota of nothing still makes progress.
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

/// A falling edge that asks for no next edge: the guest waits on something
/// only the host can bring, an input or an arrival.
pub const NO_WAKE: f64 = -1.0;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct QuantumPolicy {
    /// What is left of the frame unspent, as a share of it: the host's own
    /// work, its collector, and the turns the estimate is wrong about.
    pub margin_share: f64,
    /// The frames a turn that is not interactive may take.
    pub batch_frames: f64,
    /// How long after the last input a turn is still interactive.
    pub settle_ms: f64,
}

/// The rising edge: how long the guest may take, and why.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Grant {
    pub quota_ms: f64,
    /// The display's measured frame interval.
    pub frame_ms: f64,
    pub interactive: bool,
}

/// A turn as the host saw it, once the falling edge came back.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Turn {
    /// The gap before the turn, in seconds.
    pub delta_s: f64,
    /// The whole turn, around the call.
    pub turn_ms: f64,
    /// The grant the turn ran on.
    pub grant: Grant,
    /// When the guest wants the next edge, in ms from now: 0 the next frame,
    /// NO_WAKE none.
    pub wake_ms: f64,
}

pub struct Quantum {
    policy: QuantumPolicy,
    gaps: [f64; FRAME_WINDOW],
    gap_at: usize,
    prev_interactive: bool,
    /// What the last turn overran its quota by.
    overrun_ms: f64,
    input_at: f64,
    asked_next: bool,
}

impl Quantum {
    pub fn new(policy: QuantumPolicy) -> Quantum {
        Quantum {
            policy,
            gaps: [1000.0 / 60.0; FRAME_WINDOW],
            gap_at: 0,
            prev_interactive: false,
            overrun_ms: 0.0,
            input_at: f64::NEG_INFINITY,
            asked_next: false,
        }
    }

    /// The host saw input at `now`.
    pub fn input(&mut self, now: f64) {
        self.input_at = now;
    }

    /// Learns from a turn that finished.
    pub fn observe(&mut self, turn: Turn) {
        let gap_ms = turn.delta_s * 1000.0;
        let interactive = turn.grant.interactive;
        if interactive && self.prev_interactive && gap_ms > 0.0 && gap_ms <= MAX_FRAME_GAP_MS {
            self.gaps[self.gap_at] = gap_ms;
            self.gap_at = (self.gap_at + 1) % FRAME_WINDOW;
        }
        self.prev_interactive = interactive;
        self.overrun_ms = (turn.turn_ms - turn.grant.quota_ms).max(0.0);
        self.asked_next = turn.wake_ms == 0.0;
    }

    /// The rising edge's grant at `now`.
    pub fn grant(&self, now: f64) -> Grant {
        let mut gaps = self.gaps;
        gaps.sort_unstable_by(f64::total_cmp);
        let frame = gaps[FRAME_WINDOW / 4];
        let p = self.policy;
        let interactive = now - self.input_at < p.settle_ms || self.asked_next;
        let frames = if interactive { 1.0 } else { p.batch_frames };
        Grant {
            quota_ms: (frame * frames * (1.0 - p.margin_share) - self.overrun_ms).max(0.0),
            frame_ms: frame,
            interactive,
        }
    }
}

#[cfg(test)]
#[path = "quantum.test.rs"]
mod test;
