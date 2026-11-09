// xpute-runtime/native/pacing.rs

//! Paces the turns of a core on its own thread to the display: the display's
//! refresh as the frames show it, which frames begin a turn, and what each turn
//! may spend. @xpute/runtime sched/frame_pacing.ts is this policy on a page.

/// The program's numbers for frame pacing, as a page's runtime is given them
/// (@xpute/runtime sched/frame_pacing.ts `FramePacingConfig`).
#[derive(Clone, Copy)]
pub struct Pace {
    pub pace_hz: f64,
    pub margin_share: f64,
    pub batch_frames: f64,
    pub settle_ms: f64,
}

const WINDOW: usize = 64;
/// A gap past this between two frames is the display's having stopped.
const MAX_GAP_MS: f64 = 50.0;

/// The frame a turn was granted.
#[derive(Clone, Copy)]
pub(super) struct Grant {
    /// When the turn is to end, on the clock's ms.
    pub deadline_at: f64,
    pub refresh_ms: f64,
    pub paced_ms: f64,
    pub interactive: bool,
}

pub(super) struct Pacing {
    pace: Pace,
    /// The least interval the core asked between turns (SYS_PACE), 0 for none.
    asked_ms: f64,
    gaps: [f64; WINDOW],
    gap_at: usize,
    seen: usize,
    /// The last frame's time, and the last that began a turn.
    frame_at: f64,
    last: f64,
    input_at: f64,
    asked_next: bool,
}

impl Pacing {
    pub const fn new() -> Pacing {
        Pacing {
            pace: Pace {
                pace_hz: 60.0,
                margin_share: 0.0,
                batch_frames: 1.0,
                settle_ms: 0.0,
            },
            asked_ms: 0.0,
            gaps: [0.0; WINDOW],
            gap_at: 0,
            seen: 0,
            frame_at: f64::NEG_INFINITY,
            last: f64::NEG_INFINITY,
            input_at: f64::NEG_INFINITY,
            asked_next: false,
        }
    }

    pub fn set(&mut self, pace: Pace) {
        self.pace = pace;
    }

    pub fn ask(&mut self, interval_ms: f64) {
        self.asked_ms = interval_ms;
    }

    pub fn input(&mut self, now: f64) {
        self.input_at = now;
    }

    /// Whether the last turn answered that it wants the next at once.
    pub fn asked_next(&mut self, asked: bool) {
        self.asked_next = asked;
    }

    /// Records a display frame at `at`.
    pub fn frame(&mut self, at: f64) {
        let gap = at - self.frame_at;
        self.frame_at = at;
        if gap > 0.0 && gap <= MAX_GAP_MS {
            self.gaps[self.gap_at] = gap;
            self.gap_at = (self.gap_at + 1) % WINDOW;
            self.seen = (self.seen + 1).min(WINDOW);
        }
    }

    /// The display's refresh: the first quartile of the gaps seen, since a
    /// missed frame only lengthens a gap.
    fn refresh_ms(&self) -> f64 {
        if self.seen == 0 {
            return 1000.0 / self.pace.pace_hz;
        }
        let mut seen = self.gaps;
        let seen = &mut seen[..self.seen];
        seen.sort_unstable_by(f64::total_cmp);
        seen[self.seen >> 2]
    }

    /// The most whole refreshes within one interval at `pace_hz`, or the
    /// fewest that reach the interval the core asked for, whichever is more,
    /// and at least one. Each bound allows an eighth of a refresh for the
    /// spread of the refresh's measure.
    fn paced_ms(&self, refresh_ms: f64) -> f64 {
        let spread = refresh_ms / 8.0;
        let divisor = ((1000.0 / self.pace.pace_hz + spread) / refresh_ms)
            .floor()
            .max(((self.asked_ms - spread) / refresh_ms).ceil())
            .max(1.0);
        divisor * refresh_ms
    }

    /// Whether the frame at `at` begins a turn: it does not when it comes
    /// sooner after the last turn than the paced interval, less half a refresh
    /// for the frame's own jitter.
    pub fn admit(&mut self, at: f64) -> bool {
        let refresh_ms = self.refresh_ms();
        if at - self.last < self.paced_ms(refresh_ms) - refresh_ms / 2.0 {
            return false;
        }
        self.last = at;
        true
    }

    /// What a turn begun at the frame at `at` may spend, measured from that
    /// frame, so that a turn begun late has that much less.
    pub fn grant(&self, at: f64, now: f64) -> Grant {
        let refresh_ms = self.refresh_ms();
        let paced_ms = self.paced_ms(refresh_ms);
        let interactive = now - self.input_at < self.pace.settle_ms || self.asked_next;
        let frames = if interactive {
            1.0
        } else {
            self.pace.batch_frames
        };
        Grant {
            deadline_at: at + paced_ms * frames * (1.0 - self.pace.margin_share),
            refresh_ms,
            paced_ms,
            interactive,
        }
    }
}

#[cfg(test)]
#[path = "pacing.test.rs"]
mod test;
