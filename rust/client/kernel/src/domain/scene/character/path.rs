// pxl-kernel/domain/scene/character/path.rs

//! Companion movement math, independent of any renderer.

/// Radians, 0 = +x.
pub fn heading_to(from: [f64; 2], to: [f64; 2]) -> f64 {
    libm::atan2(to[1] - from[1], to[0] - from[0])
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WalkPlan {
    pub start: [f64; 2],
    pub target: [f64; 2],
    pub start_time: f64,
    pub duration: f64,
}

impl WalkPlan {
    /// Where it is at `now`, and its heading, none once arrived. It takes
    /// `duration` whatever the distance, so the companion catches up on the
    /// same beat after a pan of 1 unit or 1000; a fixed speed made big pans
    /// absurdly long.
    pub fn at(&self, now: f64) -> ([f64; 2], Option<f64>) {
        let frac = if self.duration > 0.0 {
            ((now - self.start_time) / self.duration).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let at = std::array::from_fn(|k| self.start[k] + (self.target[k] - self.start[k]) * frac);
        (
            at,
            (frac < 1.0).then(|| heading_to(self.start, self.target)),
        )
    }
}

/// Settles when the target stops changing, whatever moved it: keying on drag
/// state missed click-driven camera animations entirely.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SettleTracker {
    pub last_seen: [f64; 2],
    pub last_change_time: f64,
    pub settled: [f64; 2],
}

impl SettleTracker {
    pub fn new(at: [f64; 2], now: f64) -> SettleTracker {
        SettleTracker {
            last_seen: at,
            last_change_time: now,
            settled: at,
        }
    }

    /// Whether it settled just now.
    pub fn update(&mut self, target: [f64; 2], now: f64, delay: f64) -> bool {
        if target != self.last_seen {
            (self.last_seen, self.last_change_time) = (target, now);
        }
        let settles = now - self.last_change_time >= delay && self.last_seen != self.settled;
        if settles {
            self.settled = self.last_seen;
        }
        settles
    }
}
